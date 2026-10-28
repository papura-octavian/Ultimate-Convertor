mod data;
mod formats;
mod images;
mod media;
mod pdf;
#[cfg(test)]
mod tests;

use base64::Engine;
use formats::Kind;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
struct FileInfo {
    path: String,
    name: String,
    ext: String,
    size: u64,
    kind: Kind,
    targets: Vec<&'static str>,
    default_target: &'static str,
}

#[derive(Serialize)]
struct Inspected {
    files: Vec<FileInfo>,
    skipped: u32,
}

fn collect(path: &Path, depth: u32, files: &mut Vec<FileInfo>, skipped: &mut u32) {
    if path.is_dir() {
        if depth > 8 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(path) else { return };
        let mut children: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        children.sort();
        for child in children {
            collect(&child, depth + 1, files, skipped);
        }
        return;
    }
    let ext = formats::ext_of(path);
    let Some(kind) = formats::kind_of(&ext) else {
        *skipped += 1;
        return;
    };
    files.push(FileInfo {
        path: path.to_string_lossy().into_owned(),
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        targets: formats::targets(kind, &ext),
        default_target: formats::default_target(kind, &ext),
        ext,
        kind,
    });
}

/// Expands folders and keeps the files we can convert.
#[tauri::command]
fn inspect(paths: Vec<String>) -> Inspected {
    let mut files = Vec::new();
    let mut skipped = 0;
    for p in paths {
        collect(Path::new(&p), 0, &mut files, &mut skipped);
    }
    Inspected { files, skipped }
}

/// A small preview as a data URL, or nothing for files without one.
#[tauri::command]
async fn thumbnail(path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&path);
        let ext = formats::ext_of(p);
        let jpeg = match formats::kind_of(&ext)? {
            Kind::Image if formats::IMAGE_NATIVE.contains(&ext.as_str()) => images::thumbnail_jpeg(p).ok()?,
            Kind::Image => media::thumbnail_jpeg(p, false).ok()?,
            Kind::Video => media::thumbnail_jpeg(p, true).ok()?,
            _ => return None,
        };
        Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg)))
    })
    .await
    .ok()
    .flatten()
}

#[derive(Deserialize, Clone)]
struct Job {
    id: u32,
    path: String,
    target: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Options {
    quality: u8,
    max_side: u32,
    out_dir: Option<String>,
    fast: bool,
    merge_pdf: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JobEvent {
    id: u32,
    state: &'static str,
    progress: f32,
    output: Option<String>,
    out_size: Option<u64>,
    error: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BatchEvent {
    done: u32,
    failed: u32,
    canceled: u32,
    millis: u64,
}

#[derive(Default)]
struct Runner {
    running: AtomicBool,
    cancel: AtomicBool,
}

/// One unit of work: a single file, or several images merged into one PDF.
struct Task {
    jobs: Vec<Job>,
    target: String,
    output: PathBuf,
    heavy: bool,
}

/// `dir/stem.ext`, or `stem (2).ext` and so on when that is taken.
fn free_name(dir: &Path, stem: &str, ext: &str, input: &Path, taken: &mut HashSet<PathBuf>) -> PathBuf {
    let mut n = 1;
    loop {
        let name = if n == 1 { format!("{stem}.{ext}") } else { format!("{stem} ({n}).{ext}") };
        let candidate = dir.join(name);
        if candidate != input && !candidate.exists() && !taken.contains(&candidate) {
            taken.insert(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

fn plan(jobs: Vec<Job>, opts: &Options) -> Vec<Task> {
    let mut taken = HashSet::new();
    let out_dir = opts.out_dir.as_ref().map(PathBuf::from);
    let out_for =|job: &Job, stem_suffix: &str, taken: &mut HashSet<PathBuf>| {
        let input = Path::new(&job.path);
        let dir = out_dir.clone().or_else(|| input.parent().map(Path::to_path_buf)).unwrap_or_default();
        let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "output".into());
        free_name(&dir, &format!("{stem}{stem_suffix}"), &job.target, input, taken)
    };

    let mut tasks = Vec::new();
    let mut pdf_group: Vec<Job> = Vec::new();
    for job in jobs {
        let ext = formats::ext_of(Path::new(&job.path));
        let kind = formats::kind_of(&ext);
        if opts.merge_pdf && job.target == "pdf" && kind == Some(Kind::Image) {
            pdf_group.push(job);
            continue;
        }
        let heavy = kind == Some(Kind::Video) && !formats::is_audio_target(&job.target);
        let output = out_for(&job, "", &mut taken);
        tasks.push(Task { target: job.target.clone(), jobs: vec![job], output, heavy });
    }
    match pdf_group.len() {
        0 => {}
        1 => {
            let output = out_for(&pdf_group[0], "", &mut taken);
            tasks.push(Task { target: "pdf".into(), jobs: pdf_group, output, heavy: false });
        }
        n => {
            let output = out_for(&pdf_group[0], &format!(" + {}", n - 1), &mut taken);
            tasks.push(Task { target: "pdf".into(), jobs: pdf_group, output, heavy: false });
        }
    }
    tasks
}

fn run_task(task: &Task, opts: &Options, cancel: &AtomicBool, progress: &dyn Fn(f32)) -> Result<(), String> {
    let input = Path::new(&task.jobs[0].path);
    let ext = formats::ext_of(input);
    let kind = formats::kind_of(&ext).ok_or("Unsupported file")?;
    let target = task.target.as_str();
    if let Some(dir) = task.output.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let settings = media::Settings { quality: opts.quality, max_side: opts.max_side, fast: opts.fast };

    match kind {
        Kind::Image if target == "pdf" => {
            let mut pages = Vec::with_capacity(task.jobs.len());
            for (i, job) in task.jobs.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return Err("canceled".into());
                }
                let img = images::fit(images::load(Path::new(&job.path))?, opts.max_side);
                pages.push(pdf::Page::from_image(&img, opts.quality)?);
                progress((i + 1) as f32 / (task.jobs.len() + 1) as f32);
            }
            std::fs::write(&task.output, pdf::write(&pages)).map_err(|e| e.to_string())
        }
        Kind::Image if target == "avif" || formats::is_video_target(target) => {
            media::convert(input, &task.output, target, &settings, cancel, progress)
        }
        Kind::Image => {
            progress(-1.0);
            let img = images::fit(images::load(input)?, opts.max_side);
            images::save(img, target, opts.quality, &task.output)
        }
        Kind::Video | Kind::Audio => media::convert(input, &task.output, target, &settings, cancel, progress),
        Kind::Data => data::convert_data(input, &ext, target, &task.output),
        Kind::Document => data::markdown_to_html(input, &task.output),
    }
}

#[tauri::command]
fn convert(app: AppHandle, runner: State<'_, Arc<Runner>>, jobs: Vec<Job>, options: Options) -> Result<(), String> {
    if runner.running.swap(true, Ordering::SeqCst) {
        return Err("A conversion is already running".into());
    }
    runner.cancel.store(false, Ordering::SeqCst);
    let runner = Arc::clone(&runner);

    std::thread::spawn(move || {
        let started = Instant::now();
        let tasks = plan(jobs, &options);
        let (heavy, light): (Vec<Task>, Vec<Task>) = tasks.into_iter().partition(|t| t.heavy);
        let heavy = Mutex::new(VecDeque::from(heavy));
        let light = Mutex::new(VecDeque::from(light));
        let counts = Mutex::new((0u32, 0u32, 0u32));

        let emit = |task: &Task, state: &'static str, progress: f32, error: Option<String>| {
            let done = state == "done";
            let out_size = done.then(|| std::fs::metadata(&task.output).map(|m| m.len()).ok()).flatten();
            for job in &task.jobs {
                let _ = app.emit(
                    "job",
                    JobEvent {
                        id: job.id,
                        state,
                        progress,
                        output: done.then(|| task.output.to_string_lossy().into_owned()),
                        out_size,
                        error: error.clone(),
                    },
                );
            }
        };

        let worker = |queue: &Mutex<VecDeque<Task>>| loop {
            let Some(task) = queue.lock().unwrap().pop_front() else { break };
            if runner.cancel.load(Ordering::Relaxed) {
                emit(&task, "canceled", 0.0, None);
                counts.lock().unwrap().2 += task.jobs.len() as u32;
                continue;
            }
            emit(&task, "working", 0.0, None);
            let report = |p: f32| emit(&task, "working", p, None);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_task(&task, &options, &runner.cancel, &report)
            }))
            .unwrap_or_else(|_| Err("The converter crashed on this file".into()));
            let n = task.jobs.len() as u32;
            match result {
                Ok(()) => {
                    emit(&task, "done", 1.0, None);
                    counts.lock().unwrap().0 += n;
                }
                Err(e) if e == "canceled" => {
                    let _ = std::fs::remove_file(&task.output);
                    emit(&task, "canceled", 0.0, None);
                    counts.lock().unwrap().2 += n;
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&task.output);
                    emit(&task, "error", 0.0, Some(e));
                    counts.lock().unwrap().1 += n;
                }
            }
        };

        // Video encoders already use every core, so only two run at once; everything else gets a thread per core.
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        std::thread::scope(|s| {
            for _ in 0..2 {
                s.spawn(|| {
                    worker(&heavy);
                    worker(&light);
                });
            }
            for _ in 0..cores.saturating_sub(2).max(1) {
                s.spawn(|| worker(&light));
            }
        });

        let (done, failed, canceled) = *counts.lock().unwrap();
        runner.running.store(false, Ordering::SeqCst);
        let _ = app.emit("batch", BatchEvent { done, failed, canceled, millis: started.elapsed().as_millis() as u64 });
    });
    Ok(())
}

#[tauri::command]
fn cancel(runner: State<'_, Arc<Runner>>) {
    runner.cancel.store(true, Ordering::SeqCst);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(Runner::default()))
        .invoke_handler(tauri::generate_handler![inspect, thumbnail, convert, cancel])
        .run(tauri::generate_context!())
        .expect("error while running Ultimate Convertor");
}
