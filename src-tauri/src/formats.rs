use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Video,
    Audio,
    Data,
    Document,
}

/// Images the `image` crate decodes itself.
pub const IMAGE_NATIVE: &[&str] = &[
    "png", "jpg", "jpeg", "jfif", "jpe", "webp", "bmp", "gif", "tif", "tiff", "ico", "tga", "qoi", "pnm", "ppm",
    "pgm", "pbm", "pam", "hdr", "exr", "dds",
];
/// Images we hand to ffmpeg to decode.
pub const IMAGE_FFMPEG: &[&str] = &["heic", "heif", "hif", "avif"];
const VIDEO: &[&str] = &[
    "mp4", "m4v", "mkv", "mov", "avi", "webm", "wmv", "flv", "f4v", "mpg", "mpeg", "m2v", "ts", "mts", "m2ts", "3gp",
    "3g2", "ogv", "vob", "asf", "hevc", "h264", "h265", "divx", "rm", "rmvb",
];
const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "m4a", "aac", "ogg", "oga", "opus", "wma", "aiff", "aif", "aifc", "amr", "ac3", "eac3",
    "mka", "ape", "wv", "caf", "m4b", "mp2", "dts", "au", "spx",
];
const DATA: &[&str] = &["json", "yaml", "yml", "toml", "csv", "tsv"];
const DOCUMENT: &[&str] = &["md", "markdown", "mdown"];

const IMAGE_TARGETS: &[&str] = &["jpg", "png", "webp", "avif", "gif", "bmp", "tiff", "ico", "tga", "qoi", "pdf"];
const VIDEO_TARGETS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "gif", "mp3", "m4a", "wav", "flac", "ogg", "opus"];
const AUDIO_TARGETS: &[&str] = &["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff"];
const DATA_TARGETS: &[&str] = &["json", "yaml", "toml", "csv"];
const DOCUMENT_TARGETS: &[&str] = &["html"];

pub fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub fn kind_of(ext: &str) -> Option<Kind> {
    if IMAGE_NATIVE.contains(&ext) || IMAGE_FFMPEG.contains(&ext) {
        Some(Kind::Image)
    } else if VIDEO.contains(&ext) {
        Some(Kind::Video)
    } else if AUDIO.contains(&ext) {
        Some(Kind::Audio)
    } else if DATA.contains(&ext) {
        Some(Kind::Data)
    } else if DOCUMENT.contains(&ext) {
        Some(Kind::Document)
    } else {
        None
    }
}

pub fn targets(kind: Kind, ext: &str) -> Vec<&'static str> {
    let mut t = match kind {
        Kind::Image => IMAGE_TARGETS.to_vec(),
        Kind::Video => VIDEO_TARGETS.to_vec(),
        Kind::Audio => AUDIO_TARGETS.to_vec(),
        Kind::Data => DATA_TARGETS.to_vec(),
        Kind::Document => DOCUMENT_TARGETS.to_vec(),
    };
    // Animated GIFs are more useful as video than as another still.
    if ext == "gif" {
        t.extend(["mp4", "webm"]);
    }
    t
}

pub fn default_target(kind: Kind, ext: &str) -> &'static str {
    match kind {
        Kind::Image => "jpg",
        Kind::Video => "mp4",
        Kind::Audio => "mp3",
        Kind::Data if ext == "json" => "yaml",
        Kind::Data => "json",
        Kind::Document => "html",
    }
}

pub fn is_video_target(t: &str) -> bool {
    matches!(t, "mp4" | "mkv" | "webm" | "mov" | "avi")
}

pub fn is_audio_target(t: &str) -> bool {
    matches!(t, "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aiff")
}
