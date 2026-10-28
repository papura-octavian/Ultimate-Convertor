//! A minimal PDF writer: one page per image, each embedded as a JPEG at its own size.

use image::{codecs::jpeg::JpegEncoder, DynamicImage};
use std::io::Write;

pub struct Page {
    jpeg: Vec<u8>,
    width: u32,
    height: u32,
}

impl Page {
    pub fn from_image(img: &DynamicImage, quality: u8) -> Result<Page, String> {
        let rgb = crate::images::flatten_rgb(img);
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality.clamp(1, 100))
            .encode_image(&rgb)
            .map_err(|e| e.to_string())?;
        Ok(Page { jpeg, width: rgb.width(), height: rgb.height() })
    }
}

pub fn write(pages: &[Page]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // Objects: 1 catalog, 2 pages, then per page: page, contents, image.
    let page_id = |i: usize| 3 + i * 3;
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let id = offsets.len();
        write!(out, "{id} 0 obj\n").unwrap();
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };

    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", page_id(i))).collect();
    obj(&mut out, &mut offsets, format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), pages.len()).as_bytes());

    for (i, p) in pages.iter().enumerate() {
        // 96 dpi: a 1920 px wide photo becomes a 20 inch page, the way image viewers print it.
        let (w, h) = (p.width as f64 * 0.75, p.height as f64 * 0.75);
        let id = page_id(i);
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>",
                id + 2,
                id + 1
            )
            .as_bytes(),
        );
        let content = format!("q {w:.2} 0 0 {h:.2} 0 0 cm /Im0 Do Q");
        obj(&mut out, &mut offsets, format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes());
        let mut img = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            p.width,
            p.height,
            p.jpeg.len()
        )
        .into_bytes();
        img.extend_from_slice(&p.jpeg);
        img.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offsets, &img);
    }

    let xref = out.len();
    write!(out, "xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).unwrap();
    for o in &offsets {
        write!(out, "{o:010} 00000 n \n").unwrap();
    }
    write!(out, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).unwrap();
    out
}
