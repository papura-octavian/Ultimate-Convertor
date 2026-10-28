//! End-to-end conversions on generated samples. Needs ffmpeg in `binaries/` (see README).

use super::*;
use std::process::Command;

fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("ultimate-convertor-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn opts(out: &Path) -> Options {
    Options { quality: 80, max_side: 0, out_dir: Some(out.to_string_lossy().into_owned()), fast: true, merge_pdf: false }
}

fn job(id: u32, path: &Path, target: &str) -> Job {
    Job { id, path: path.to_string_lossy().into_owned(), target: target.into() }
}

fn run_one(path: &Path, target: &str, o: &Options) -> Result<PathBuf, String> {
    let tasks = plan(vec![job(1, path, target)], o);
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    run_task(&tasks[0], o, &cancel, &|_| {})?;
    let size = std::fs::metadata(&tasks[0].output).map(|m| m.len()).unwrap_or(0);
    println!("{:>28} -> {:<5} {:>9} B  {:>5} ms", path.file_name().unwrap().to_string_lossy(), target, size, started.elapsed().as_millis());
    assert!(size > 0, "{target}: empty output");
    Ok(tasks[0].output.clone())
}

fn ffmpeg_make(args: &[&str], out: &Path) {
    let status = Command::new(media::ffmpeg_path()).args(["-hide_banner", "-v", "error", "-y"]).args(args).arg(out).status().unwrap();
    assert!(status.success(), "ffmpeg sample generation failed: {args:?}");
}

#[test]
fn images() {
    let dir = workdir("images");
    let img = image::RgbaImage::from_fn(1200, 800, |x, y| image::Rgba([(x / 5) as u8, (y / 4) as u8, 160, if x < 100 { 120 } else { 255 }]));
    let png = dir.join("sample.png");
    img.save(&png).unwrap();
    let out = dir.join("out");
    let o = opts(&out);
    for target in ["jpg", "png", "webp", "avif", "gif", "bmp", "tiff", "ico", "tga", "qoi", "pdf"] {
        let result = run_one(&png, target, &o).unwrap_or_else(|e| panic!("png -> {target}: {e}"));
        if !matches!(target, "pdf" | "avif") {
            let back = images::load(&result).unwrap_or_else(|e| panic!("reopen {target}: {e}"));
            let expect = if target == "ico" { (256, 171) } else { (1200, 800) };
            assert_eq!((back.width(), back.height()), expect, "{target} size");
        }
    }
    // Decoding through ffmpeg (the HEIC/AVIF path) and resizing.
    let avif = out.join("sample.avif");
    let mut small = opts(&out);
    small.max_side = 300;
    let jpg = run_one(&avif, "jpg", &small).expect("avif -> jpg");
    let back = images::load(&jpg).unwrap();
    assert_eq!((back.width(), back.height()), (300, 200));
    let webp = out.join("sample.webp");
    run_one(&webp, "png", &o).expect("webp -> png");
}

#[test]
fn merge_pdf() {
    let dir = workdir("pdf");
    let mut jobs = Vec::new();
    for i in 0..3 {
        let p = dir.join(format!("page{i}.jpg"));
        image::RgbImage::from_pixel(600 + i * 100, 400, image::Rgb([40 * i as u8, 90, 200])).save(&p).unwrap();
        jobs.push(job(i, &p, "pdf"));
    }
    let mut o = opts(&dir.join("out"));
    o.merge_pdf = true;
    let tasks = plan(jobs, &o);
    assert_eq!(tasks.len(), 1);
    run_task(&tasks[0], &o, &AtomicBool::new(false), &|_| {}).unwrap();
    let bytes = std::fs::read(&tasks[0].output).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("%PDF-1.4") && text.contains("/Count 3") && text.trim_end().ends_with("%%EOF"));
    println!("merged pdf: {} ({} B)", tasks[0].output.display(), bytes.len());
}

#[test]
fn video_and_audio() {
    let dir = workdir("media");
    let mp4 = dir.join("clip.mp4");
    ffmpeg_make(
        &[
            "-f", "lavfi", "-i", "testsrc2=duration=4:size=1280x720:rate=30", "-f", "lavfi", "-i", "sine=frequency=440:duration=4",
            "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest",
        ],
        &mp4,
    );
    assert!(mp4.exists());

    let out = dir.join("out");
    let o = opts(&out);
    let progress_seen = std::sync::atomic::AtomicU32::new(0);
    for target in ["mkv", "mov", "mp4", "webm", "avi", "gif", "mp3", "m4a", "wav", "flac", "ogg", "opus"] {
        let tasks = plan(vec![job(1, &mp4, target)], &o);
        let started = Instant::now();
        run_task(&tasks[0], &o, &AtomicBool::new(false), &|p| {
            if p > 0.0 {
                progress_seen.fetch_add(1, Ordering::Relaxed);
            }
        })
        .unwrap_or_else(|e| panic!("mp4 -> {target}: {e}"));
        let size = std::fs::metadata(&tasks[0].output).unwrap().len();
        println!("clip.mp4 -> {target:<5} {size:>9} B  {:>5} ms", started.elapsed().as_millis());
        assert!(size > 1000);
    }
    assert!(progress_seen.load(Ordering::Relaxed) > 0, "no progress reported");

    // Re-encode (not remux) with a size limit.
    let mut small = opts(&out);
    small.max_side = 640;
    let p = run_one(&mp4, "mkv", &small).unwrap();
    let probe = media::probe(&p);
    assert_eq!(probe.video.as_deref(), Some("h264"));

    // Audio-only sources.
    let wav = out.join("clip.wav");
    for target in ["mp3", "flac", "m4a", "aiff", "opus"] {
        run_one(&wav, target, &o).unwrap_or_else(|e| panic!("wav -> {target}: {e}"));
    }
    assert!(run_one(&wav, "mp4", &o).is_err(), "audio to video must fail cleanly");

    // Animated GIF to video.
    let gif = out.join("clip.gif");
    run_one(&gif, "mp4", &o).expect("gif -> mp4");
}

#[test]
fn cancel_stops_ffmpeg() {
    let dir = workdir("cancel");
    let src = dir.join("long.mkv");
    ffmpeg_make(&["-f", "lavfi", "-i", "testsrc2=duration=60:size=1920x1080:rate=30", "-c:v", "libx264", "-preset", "ultrafast"], &src);
    let o = Options { fast: false, ..opts(&dir.join("out")) };
    let tasks = plan(vec![job(1, &src, "webm")], &o);
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(700));
            cancel.store(true, Ordering::SeqCst);
        });
        let r = run_task(&tasks[0], &o, &cancel, &|_| {});
        assert_eq!(r, Err("canceled".to_string()));
    });
    assert!(started.elapsed().as_secs() < 5);
    assert!(!tasks[0].output.exists(), "partial output left behind");
}

#[test]
fn data_and_docs() {
    let dir = workdir("data");
    let json = dir.join("people.json");
    std::fs::write(&json, r#"{"people":[{"name":"Ana","age":31,"phone":"0721"},{"name":"Ion","age":40,"tags":["a","b"],"note":null}]}"#).unwrap();
    let out = dir.join("out");
    let o = opts(&out);
    for target in ["yaml", "toml", "csv", "json"] {
        run_one(&json, target, &o).unwrap_or_else(|e| panic!("json -> {target}: {e}"));
    }
    let csv = std::fs::read_to_string(out.join("people.csv")).unwrap();
    println!("{csv}");
    assert!(csv.contains("name,age,phone,tags,note"));
    // CSV back to JSON keeps numbers, and leading zeros as text.
    let back = run_one(&out.join("people.csv"), "json", &o).unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(back).unwrap()).unwrap();
    assert_eq!(v[0]["age"], 31);
    assert_eq!(v[0]["phone"], "0721");
    // YAML and TOML round trip.
    run_one(&out.join("people.yaml"), "json", &o).unwrap();
    run_one(&out.join("people.toml"), "yaml", &o).unwrap();

    let md = dir.join("notes.md");
    std::fs::write(&md, "# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n").unwrap();
    let html = std::fs::read_to_string(run_one(&md, "html", &o).unwrap()).unwrap();
    assert!(html.contains("<h1>Title</h1>") && html.contains("<table>"));
}

#[test]
fn names_never_collide() {
    let dir = workdir("names");
    let a = dir.join("photo.png");
    let b = dir.join("photo.jpg");
    std::fs::write(&a, b"x").unwrap();
    std::fs::write(&b, b"x").unwrap();
    std::fs::write(dir.join("photo.webp"), b"x").unwrap();
    let o = Options { out_dir: None, ..opts(&dir) };
    let tasks = plan(vec![job(1, &a, "webp"), job(2, &b, "webp"), job(3, &b, "jpg")], &o);
    let names: Vec<String> = tasks.iter().map(|t| t.output.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["photo (2).webp", "photo (3).webp", "photo (2).jpg"]);
}
