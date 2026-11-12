//! Platform-independent helpers for saving the drawing screen as PNG / JSON:
//! date + sequence file names, PNG encode/decode and the base64 image field.

use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

pub const JSON_FORMAT: &str = "keyviz-drawing";
pub const JSON_VERSION: u32 = 1;

/// Highest `NN` among files named `{date}-NN.{ext}` in `dir` (0 when none).
fn highest_sequence(dir: &Path, date: &str, ext: &str) -> u32 {
    let prefix = format!("{date}-");
    let suffix = format!(".{ext}");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_ascii_lowercase();
            name.strip_prefix(&prefix)?
                .strip_suffix(&suffix)?
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0)
}

/// Creates `{dir}/{date}-NN.{ext}` with the next free sequence number and writes `bytes`.
pub fn write_numbered(dir: &Path, date: &str, ext: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    let mut next = highest_sequence(dir, date, ext) + 1;
    for _ in 0..1000 {
        let path = dir.join(format!("{date}-{next:02}.{ext}"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(bytes)
                    .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => next += 1,
            Err(error) => return Err(format!("Cannot create {}: {error}", path.display())),
        }
    }
    Err("No free file name left for today".into())
}

pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err("PNG buffer size does not match the image size".into());
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Decodes any PNG into 8-bit RGBA.
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    buffer.truncate(info.buffer_size());
    let pixels = info.width as usize * info.height as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("Indexed PNG was not expanded".into()),
    };
    if rgba.len() != pixels * 4 {
        return Err("Unexpected PNG pixel data".into());
    }
    Ok((info.width, info.height, rgba))
}

pub fn png_to_base64(png: &[u8]) -> String {
    STANDARD.encode(png)
}

pub fn base64_to_bytes(text: &str) -> Result<Vec<u8>, String> {
    STANDARD.decode(text.trim()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keyviz-export-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn numbering_continues_per_extension() {
        let dir = temp_dir("numbering");
        let first = write_numbered(&dir, "20261112", "png", b"a").unwrap();
        let second = write_numbered(&dir, "20261112", "png", b"b").unwrap();
        let json = write_numbered(&dir, "20261112", "json", b"c").unwrap();
        std::fs::write(dir.join("20261112-09.png"), b"x").unwrap();
        let after_gap = write_numbered(&dir, "20261112", "png", b"d").unwrap();
        let next_day = write_numbered(&dir, "20261113", "png", b"e").unwrap();
        assert_eq!(first.file_name().unwrap(), "20261112-01.png");
        assert_eq!(second.file_name().unwrap(), "20261112-02.png");
        assert_eq!(json.file_name().unwrap(), "20261112-01.json");
        assert_eq!(after_gap.file_name().unwrap(), "20261112-10.png");
        assert_eq!(next_day.file_name().unwrap(), "20261113-01.png");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn png_round_trip_keeps_pixels() {
        let rgba: Vec<u8> = (0..4 * 3 * 4).map(|i| (i * 7 % 256) as u8).collect();
        let png = encode_png(4, 3, &rgba).unwrap();
        let (w, h, decoded) = decode_png(&png).unwrap();
        assert_eq!((w, h), (4, 3));
        assert_eq!(decoded, rgba);
        let text = png_to_base64(&png);
        assert_eq!(base64_to_bytes(&text).unwrap(), png);
        assert!(encode_png(4, 3, &rgba[1..]).is_err());
    }
}
