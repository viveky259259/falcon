//! Crop a PNG to its top-left corner, for screenshots that include content
//! painted outside the screen.

use anyhow::{Context, Result};

/// Keep the top-left `max_width` × `max_height` pixels of a PNG. Returns
/// `None` when the image already fits, so the caller can keep the original
/// bytes.
pub fn crop_top_left(png_bytes: &[u8], max_width: u32, max_height: u32) -> Result<Option<Vec<u8>>> {
    let mut reader = png::Decoder::new(png_bytes)
        .read_info()
        .context("The screenshot is not a readable PNG")?;
    let (width, height) = {
        let info = reader.info();
        (info.width, info.height)
    };
    if max_width == 0 || max_height == 0 || (width <= max_width && height <= max_height) {
        return Ok(None);
    }
    let mut pixels = vec![0; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut pixels)
        .context("The screenshot PNG could not be decoded")?;
    let row_len = frame.line_size;
    let pixel_len = row_len / width as usize;
    let out_width = width.min(max_width);
    let out_height = height.min(max_height);
    let keep = out_width as usize * pixel_len;
    let mut cropped = Vec::with_capacity(keep * out_height as usize);
    for row in pixels.chunks_exact(row_len).take(out_height as usize) {
        cropped.extend_from_slice(&row[..keep]);
    }

    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, out_width, out_height);
    encoder.set_color(frame.color_type);
    encoder.set_depth(frame.bit_depth);
    let mut writer = encoder
        .write_header()
        .context("The cropped screenshot could not be encoded")?;
    writer
        .write_image_data(&cropped)
        .context("The cropped screenshot could not be encoded")?;
    writer
        .finish()
        .context("The cropped screenshot could not be encoded")?;
    Ok(Some(out))
}

/// The size to crop an inspector screenshot to, or `None` when it already
/// fits. `screen` is the logical size and device pixel ratio, `image` is the
/// PNG's size and `scale` is how many image pixels one physical pixel takes.
pub fn screen_crop(screen: (f64, f64, f64), image: (u32, u32), scale: f64) -> Option<(u32, u32)> {
    let (logical_width, logical_height, ratio) = screen;
    let width = (logical_width * ratio * scale).round();
    let height = (logical_height * ratio * scale).round();
    if !(width >= 1.0 && height >= 1.0) {
        return None;
    }
    let (width, height) = (width as u32, height as u32);
    (image.0 > width || image.1 > height).then_some((width, height))
}

/// Width and height from a PNG's header.
pub fn dimensions(png_bytes: &[u8]) -> Option<(u32, u32)> {
    let header = png_bytes.get(..24)?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return None;
    }
    let read = |at: usize| {
        u32::from_be_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    Some((read(16), read(20)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An RGBA image whose pixel at (x, y) is `[x, y, 0, 255]`.
    fn sample(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::new();
        for y in 0..height {
            for x in 0..width {
                data.extend_from_slice(&[x as u8, y as u8, 0, 255]);
            }
        }
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();
        out
    }

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(bytes).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        pixels.truncate(frame.buffer_size());
        (frame.width, frame.height, pixels)
    }

    #[test]
    fn overflow_below_the_screen_is_cut_off() {
        // A 4x3 screen whose content overflowed to 4x9.
        let cropped = crop_top_left(&sample(4, 9), 4, 3).unwrap().unwrap();
        let (width, height, pixels) = decode(&cropped);
        assert_eq!((width, height), (4, 3));
        assert_eq!(&pixels[..4], &[0, 0, 0, 255]);
        // Last pixel kept is (3, 2): the visible bottom-right corner.
        assert_eq!(&pixels[pixels.len() - 4..], &[3, 2, 0, 255]);
        assert_eq!(dimensions(&cropped), Some((4, 3)));
    }

    #[test]
    fn overflow_to_the_right_is_cut_off() {
        let cropped = crop_top_left(&sample(7, 2), 5, 2).unwrap().unwrap();
        let (width, height, pixels) = decode(&cropped);
        assert_eq!((width, height), (5, 2));
        assert_eq!(&pixels[16..20], &[4, 0, 0, 255]);
        assert_eq!(&pixels[20..24], &[0, 1, 0, 255]);
    }

    #[test]
    fn an_image_that_fits_is_left_alone() {
        assert!(crop_top_left(&sample(4, 3), 4, 3).unwrap().is_none());
        assert!(crop_top_left(&sample(4, 3), 8, 8).unwrap().is_none());
        assert!(crop_top_left(b"not a png", 4, 3).is_err());
    }

    #[test]
    fn screen_crop_uses_physical_pixels() {
        // 400x300 logical at 2x is 800x600 physical; the image is taller.
        assert_eq!(
            screen_crop((400.0, 300.0, 2.0), (800, 1400), 1.0),
            Some((800, 600))
        );
        assert_eq!(screen_crop((400.0, 300.0, 2.0), (800, 600), 1.0), None);
        // An image taken at half scale shows the screen in half the pixels.
        assert_eq!(
            screen_crop((400.0, 300.0, 2.0), (400, 9000), 0.5),
            Some((400, 300))
        );
        assert_eq!(screen_crop((0.0, 0.0, 0.0), (800, 1400), 1.0), None);
    }

    #[test]
    fn dimensions_reads_the_header() {
        assert_eq!(dimensions(&sample(6, 5)), Some((6, 5)));
        assert_eq!(dimensions(b"short"), None);
    }
}
