use crate::{formats, media};
use image::{
    codecs::{gif::GifEncoder, jpeg::JpegEncoder},
    imageops::FilterType,
    DynamicImage, ExtendedColorType, ImageDecoder, ImageFormat, ImageReader, RgbImage,
};
use std::{fs::File, io::BufWriter, io::Write, path::Path};

/// Decodes any supported still image, upright (EXIF orientation applied).
pub fn load(path: &Path) -> Result<DynamicImage, String> {
    let ext = formats::ext_of(path);
    if formats::IMAGE_FFMPEG.contains(&ext.as_str()) {
        return load_with_ffmpeg(path);
    }
    // Odd files (CMYK JPEGs, exotic TIFFs) the image crate rejects still open through ffmpeg.
    load_native(path).or_else(|native_err| load_with_ffmpeg(path).map_err(|_| native_err))
}

fn load_native(path: &Path) -> Result<DynamicImage, String> {
    let mut reader = ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    reader.no_limits();
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    img.apply_orientation(orientation);
    Ok(img)
}

fn load_with_ffmpeg(path: &Path) -> Result<DynamicImage, String> {
    let png = media::decode_still(path)?;
    image::load_from_memory_with_format(&png, ImageFormat::Png).map_err(|e| e.to_string())
}

/// Shrinks so the longest side is at most `max_side`; never enlarges.
pub fn fit(img: DynamicImage, max_side: u32) -> DynamicImage {
    if max_side == 0 || (img.width() <= max_side && img.height() <= max_side) {
        return img;
    }
    img.resize(max_side, max_side, FilterType::CatmullRom)
}

/// RGB with any transparency composited onto white (for JPEG and PDF).
pub fn flatten_rgb(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u32;
        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
        image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])])
    })
}

/// 8 bits per channel, keeping alpha only when the image has it.
fn to_8bit(img: DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) | DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_) => img,
        _ if img.color().has_alpha() => DynamicImage::ImageRgba8(img.to_rgba8()),
        _ => DynamicImage::ImageRgb8(img.to_rgb8()),
    }
}

pub fn save(img: DynamicImage, target: &str, quality: u8, out: &Path) -> Result<(), String> {
    let q = quality.clamp(1, 100);
    let file = File::create(out).map_err(|e| e.to_string())?;
    let mut w = BufWriter::new(file);
    let err = |e: image::ImageError| e.to_string();
    match target {
        "jpg" => {
            let rgb = flatten_rgb(&img);
            JpegEncoder::new_with_quality(&mut w, q).encode_image(&rgb).map_err(err)?;
        }
        "webp" => {
            let img = to_8bit(img);
            let (wd, ht) = (img.width(), img.height());
            if wd > 16383 || ht > 16383 {
                return Err("WebP is limited to 16383 px per side; set a max size".into());
            }
            let mem = if img.color().has_alpha() {
                let rgba = img.to_rgba8();
                let enc = webp::Encoder::from_rgba(&rgba, wd, ht);
                if q >= 100 { enc.encode_lossless() } else { enc.encode(q as f32) }
            } else {
                let rgb = img.to_rgb8();
                let enc = webp::Encoder::from_rgb(&rgb, wd, ht);
                if q >= 100 { enc.encode_lossless() } else { enc.encode(q as f32) }
            };
            w.write_all(&mem).map_err(|e| e.to_string())?;
        }
        "gif" => {
            let rgba = img.to_rgba8();
            GifEncoder::new_with_speed(&mut w, 10)
                .encode(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
                .map_err(err)?;
        }
        "png" | "tiff" => {
            // Both keep 16-bit depth; float images (HDR, EXR) go down to 8 bits.
            let img = match img {
                DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => to_8bit(img),
                other => other,
            };
            let fmt = if target == "png" { ImageFormat::Png } else { ImageFormat::Tiff };
            img.write_to(&mut w, fmt).map_err(err)?;
        }
        "ico" => to_8bit(fit(img, 256)).write_to(&mut w, ImageFormat::Ico).map_err(err)?,
        "bmp" => to_8bit(img).write_to(&mut w, ImageFormat::Bmp).map_err(err)?,
        "tga" => to_8bit(img).write_to(&mut w, ImageFormat::Tga).map_err(err)?,
        "qoi" => {
            let img = if img.color().has_alpha() { DynamicImage::ImageRgba8(img.to_rgba8()) } else { DynamicImage::ImageRgb8(img.to_rgb8()) };
            img.write_to(&mut w, ImageFormat::Qoi).map_err(err)?
        }
        other => return Err(format!("Unsupported image target: {other}")),
    }
    w.flush().map_err(|e| e.to_string())
}

/// A small JPEG preview for the file list.
pub fn thumbnail_jpeg(path: &Path) -> Result<Vec<u8>, String> {
    let img = load(path)?.thumbnail(144, 144);
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 78).encode_image(&flatten_rgb(&img)).map_err(|e| e.to_string())?;
    Ok(out)
}
