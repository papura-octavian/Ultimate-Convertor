//! Everything that goes through the bundled ffmpeg.

use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, OnceLock,
    },
    thread,
    time::Duration,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub(crate) fn ffmpeg_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        // Tauri puts the sidecar next to the app's own exe.
        let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.join(name)))
            .filter(|p| p.exists())
            .or_else(dev_sidecar)
            .unwrap_or_else(|| PathBuf::from(name))
    })
}

/// Tests and `cargo run` don't get the sidecar copied next to them; use the one in `binaries/`.
fn dev_sidecar() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).find(|p| {
        p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("ffmpeg-"))
    })
}

fn ffmpeg() -> Command {
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args(["-hide_banner", "-nostdin"]);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

fn run_capture(mut cmd: Command) -> Result<Vec<u8>, String> {
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    if out.status.success() && !out.stdout.is_empty() {
        Ok(out.stdout)
    } else {
        Err(last_error(&String::from_utf8_lossy(&out.stderr)))
    }
}

/// First frame of an image or video as PNG bytes.
pub fn decode_still(path: &Path) -> Result<Vec<u8>, String> {
    let mut cmd = ffmpeg();
    cmd.args(["-v", "error", "-i"]).arg(path).args(["-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "-"]);
    run_capture(cmd)
}

/// A small JPEG preview; for videos, a frame one second in when there is one.
pub fn thumbnail_jpeg(path: &Path, video: bool) -> Result<Vec<u8>, String> {
    let shot = |seek: bool| {
        let mut cmd = ffmpeg();
        cmd.args(["-v", "error"]);
        if seek {
            cmd.args(["-ss", "1"]);
        }
        cmd.arg("-i").arg(path).args([
            "-frames:v", "1", "-vf", "scale=144:144:force_original_aspect_ratio=decrease", "-f", "image2pipe", "-c:v",
            "mjpeg", "-q:v", "5", "-",
        ]);
        run_capture(cmd)
    };
    if video {
        shot(true).or_else(|_| shot(false))
    } else {
        shot(false)
    }
}

#[derive(Default, Debug)]
pub struct Probe {
    pub duration: Option<f64>,
    pub video: Option<String>,
    pub audio: Option<String>,
}

/// Reads duration and codecs from ffmpeg's banner (ffmpeg -i without an output).
pub fn probe(path: &Path) -> Probe {
    let mut cmd = ffmpeg();
    cmd.arg("-i").arg(path);
    let Ok(out) = cmd.stdin(Stdio::null()).output() else { return Probe::default() };
    let text = String::from_utf8_lossy(&out.stderr);
    let mut p = Probe::default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Duration: ") {
            p.duration = parse_clock(rest.split(',').next().unwrap_or(""));
        } else if line.starts_with("Stream #") {
            let codec = |tag: &str| {
                line.split(tag).nth(1).and_then(|r| r.split([' ', ',', '(']).next()).map(|c| c.trim().to_string())
            };
            if p.video.is_none() && line.contains(" Video: ") && !line.contains("attached pic") {
                p.video = codec(" Video: ");
            } else if p.audio.is_none() && line.contains(" Audio: ") {
                p.audio = codec(" Audio: ");
            }
        }
    }
    p
}

fn parse_clock(s: &str) -> Option<f64> {
    let mut parts = s.trim().split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let sec: f64 = parts.next()?.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// The most useful line of ffmpeg's error output.
fn last_error(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines
        .iter()
        .rev()
        .find(|l| {
            let l = l.to_ascii_lowercase();
            (l.contains("error") || l.contains("invalid") || l.contains("not ") || l.contains("unsupported") || l.contains("could"))
                && !l.starts_with("conversion failed")
        })
        .or(lines.last())
        .map(|l| l.to_string())
        .unwrap_or_else(|| "ffmpeg failed".into())
}

pub struct Settings {
    pub quality: u8,
    pub max_side: u32,
    pub fast: bool,
}

fn scale_filter(max_side: u32) -> Option<String> {
    (max_side > 0).then(|| {
        format!(
            "scale='min({m},iw)':'min({m},ih)':force_original_aspect_ratio=decrease,scale=trunc(iw/2)*2:trunc(ih/2)*2",
            m = max_side
        )
    })
}

fn audio_bitrate(q: u8, opus: bool) -> &'static str {
    match (q, opus) {
        (90.., false) => "320k",
        (75..=89, false) => "256k",
        (55..=74, false) => "192k",
        (35..=54, false) => "160k",
        (_, false) => "128k",
        (90.., true) => "192k",
        (75..=89, true) => "160k",
        (55..=74, true) => "128k",
        (35..=54, true) => "96k",
        (_, true) => "64k",
    }
}

fn audio_args(target: &str, q: u8, src_audio: Option<&str>, fast: bool) -> Vec<String> {
    let copy = fast
        && matches!(
            (target, src_audio),
            ("mp3", Some("mp3")) | ("m4a", Some("aac")) | ("opus", Some("opus")) | ("flac", Some("flac")) | ("ogg", Some("vorbis"))
        );
    let a: &[&str] = if copy {
        &["-c:a", "copy"]
    } else {
        match target {
            "mp3" => &["-c:a", "libmp3lame", "-b:a", audio_bitrate(q, false)],
            "m4a" => &["-c:a", "aac", "-b:a", if q >= 90 { "256k" } else { audio_bitrate(q, false) }],
            "opus" => &["-c:a", "libopus", "-b:a", audio_bitrate(q, true)],
            "ogg" => &["-c:a", "libvorbis", "-q:a", if q >= 90 { "8" } else if q >= 70 { "6" } else { "4" }],
            "flac" => &["-c:a", "flac"],
            "wav" => &["-c:a", "pcm_s16le"],
            "aiff" => &["-c:a", "pcm_s16be"],
            _ => &[],
        }
    };
    let mut v: Vec<String> = ["-vn", "-sn", "-map", "0:a:0"].iter().map(|s| s.to_string()).collect();
    v.extend(a.iter().map(|s| s.to_string()));
    if target == "m4a" {
        v.extend(["-movflags".into(), "+faststart".into()]);
    }
    v
}

/// Container-only change when the streams already fit the target: seconds instead of minutes.
fn remux_args(target: &str, probe: &Probe) -> Option<Vec<String>> {
    let video = probe.video.as_deref()?;
    let audio = probe.audio.as_deref();
    let ok = match target {
        "mkv" => true,
        "mp4" | "mov" => {
            matches!(video, "h264" | "hevc" | "av1" | "mpeg4")
                && audio.is_none_or(|a| matches!(a, "aac" | "mp3" | "ac3" | "eac3" | "alac" | "opus"))
        }
        "webm" => matches!(video, "vp8" | "vp9" | "av1") && audio.is_none_or(|a| matches!(a, "opus" | "vorbis")),
        _ => false,
    };
    if !ok {
        return None;
    }
    let mut v: Vec<String> = ["-map", "0:v:0", "-map", "0:a?", "-c", "copy"].iter().map(|s| s.to_string()).collect();
    if target == "mkv" {
        v.extend(["-map".into(), "0:s?".into()]);
    }
    if matches!(target, "mp4" | "mov") {
        v.extend(["-movflags".into(), "+faststart".into()]);
        if video == "hevc" {
            v.extend(["-tag:v".into(), "hvc1".into()]); // so Apple/Windows players accept it
        }
    }
    Some(v)
}

fn video_args(target: &str, s: &Settings, has_audio: bool) -> Vec<String> {
    let q = s.quality as f32;
    let x264_crf = (28.0 - (q - 50.0) * 0.2).round().clamp(16.0, 40.0).to_string();
    let vp9_crf = (45.0 - (q - 50.0) * 0.3).round().clamp(15.0, 55.0).to_string();
    let mut v: Vec<String> = Vec::new();
    let mut push = |a: &[&str]| v.extend(a.iter().map(|s| s.to_string()));
    push(&["-map", "0:v:0"]);
    if has_audio && target != "gif" {
        push(&["-map", "0:a:0?"]);
    }
    let mut filters: Vec<String> = Vec::new();
    match target {
        "gif" => {
            let side = if s.max_side > 0 { s.max_side.min(1280) } else { 640 };
            return vec![
                "-map".into(),
                "0:v:0".into(),
                "-an".into(),
                "-vf".into(),
                format!(
                    "fps=15,scale='min({side},iw)':'min({side},ih)':force_original_aspect_ratio=decrease:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4"
                ),
                "-loop".into(),
                "0".into(),
            ];
        }
        "webm" => {
            push(&["-c:v", "libvpx-vp9", "-crf", &vp9_crf, "-b:v", "0", "-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1"]);
            push(&["-c:a", "libopus", "-b:a", audio_bitrate(s.quality, true)]);
        }
        "avi" => {
            let mq = (2.0 + (100.0 - q) / 12.0).round().clamp(1.0, 12.0).to_string();
            push(&["-c:v", "mpeg4", "-vtag", "xvid", "-q:v", &mq, "-c:a", "libmp3lame", "-b:a", audio_bitrate(s.quality, false)]);
        }
        _ => {
            // mp4, mov, mkv: H.264 plays everywhere.
            push(&["-c:v", "libx264", "-preset", "veryfast", "-crf", &x264_crf, "-pix_fmt", "yuv420p"]);
            push(&["-c:a", "aac", "-b:a", if s.quality >= 75 { "192k" } else { "160k" }]);
            if target != "mkv" {
                push(&["-movflags", "+faststart"]);
            }
        }
    }
    if let Some(f) = scale_filter(s.max_side) {
        filters.push(f);
    } else if target != "webm" {
        // yuv420p needs even dimensions.
        filters.push("scale=trunc(iw/2)*2:trunc(ih/2)*2".into());
    }
    if !filters.is_empty() {
        v.push("-vf".into());
        v.push(filters.join(","));
    }
    v
}

fn avif_args(s: &Settings) -> Vec<String> {
    let crf = (40.0 - (s.quality as f32 - 50.0) * 0.5).round().clamp(10.0, 55.0).to_string();
    let mut v: Vec<String> = ["-frames:v", "1", "-c:v", "libaom-av1", "-still-picture", "1", "-crf", &crf, "-cpu-used", "6", "-row-mt", "1", "-pix_fmt", "yuv420p"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let even = "scale=trunc(iw/2)*2:trunc(ih/2)*2".to_string();
    v.extend(["-vf".into(), scale_filter(s.max_side).unwrap_or(even)]);
    v
}

/// Converts `input` to `output`. `progress` gets 0..1, or a negative value when the length is unknown.
pub fn convert(
    input: &Path,
    output: &Path,
    target: &str,
    s: &Settings,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<(), String> {
    let p = probe(input);
    let src_is_audio_only = p.video.is_none();

    let mut attempts: Vec<Vec<String>> = Vec::new();
    if target == "avif" {
        attempts.push(avif_args(s));
    } else if crate::formats::is_audio_target(target) {
        if p.audio.is_none() {
            return Err("This file has no audio track".into());
        }
        attempts.push(audio_args(target, s.quality, p.audio.as_deref(), s.fast));
        if s.fast {
            attempts.push(audio_args(target, s.quality, p.audio.as_deref(), false));
        }
    } else {
        if src_is_audio_only {
            return Err("This file has no video track".into());
        }
        // Same container in and out means the user wants it smaller, so always re-encode then.
        if s.fast && s.max_side == 0 && crate::formats::ext_of(input) != target {
            if let Some(r) = remux_args(target, &p) {
                attempts.push(r);
            }
        }
        attempts.push(video_args(target, s, p.audio.is_some()));
    }
    attempts.dedup();

    let mut last_err = String::new();
    for args in attempts {
        if cancel.load(Ordering::Relaxed) {
            return Err("canceled".into());
        }
        match run(input, output, &args, p.duration, cancel, progress) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let _ = std::fs::remove_file(output);
                if e == "canceled" {
                    return Err(e);
                }
                last_err = e;
            }
        }
    }
    Err(last_err)
}

fn run(
    input: &Path,
    output: &Path,
    args: &[String],
    duration: Option<f64>,
    cancel: &AtomicBool,
    progress: &dyn Fn(f32),
) -> Result<(), String> {
    let mut cmd = ffmpeg();
    cmd.args(["-y", "-v", "error", "-progress", "pipe:1", "-nostats", "-i"])
        .arg(input)
        .args(args)
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Could not start ffmpeg: {e}"))?;

    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let err_reader = thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let (tx, rx) = mpsc::channel::<f32>();
    let out_reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(us) = line.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<f64>().ok()) {
                if let Some(d) = duration.filter(|d| *d > 0.0) {
                    let _ = tx.send((us / 1e6 / d).clamp(0.0, 0.999) as f32);
                }
            }
        }
    });

    progress(if duration.is_some() { 0.0 } else { -1.0 });
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out_reader.join();
            let _ = err_reader.join();
            return Err("canceled".into());
        }
        let mut latest = None;
        while let Ok(v) = rx.try_recv() {
            latest = Some(v);
        }
        if let Some(v) = latest {
            progress(v);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(80)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = out_reader.join();
    let stderr = err_reader.join().unwrap_or_default();
    if status.success() && output.exists() {
        Ok(())
    } else {
        Err(last_error(&stderr))
    }
}
