use base64::{engine::general_purpose::STANDARD, Engine};

/// Encode opaque, top-down GDI BGRA pixels as a PNG, then plain Base64 text.
pub fn encode(pixels: &[u8], width: i32, height: i32) -> Result<String, String> {
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4));
    if width <= 0 || height <= 0 || expected != Some(pixels.len()) {
        return Err("Invalid screenshot dimensions or pixel data.".into());
    }
    // GDI's fourth byte is reserved and often zero; export opaque RGB.
    let rgb: Vec<u8> = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect();
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&rgb).map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
    }
    Ok(wrap_base64(&png))
}

fn wrap_base64(bytes: &[u8]) -> String {
    let encoded = STANDARD.encode(bytes);
    let mut text = String::with_capacity(encoded.len() + encoded.len().div_ceil(76));
    for line in encoded.as_bytes().chunks(76) {
        // Standard Base64 is ASCII. LF matches `base64` output, including the final newline.
        text.push_str(std::str::from_utf8(line).expect("Base64 is ASCII"));
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_76_and_round_trips_across_terminal_limit() {
        for size in [1, 2, 3, 56, 57, 58, 114, 4096, 8192] {
            let bytes: Vec<u8> = (0..size).map(|n| n as u8).collect();
            let text = wrap_base64(&bytes);
            assert!(text.ends_with('\n'));
            let lines: Vec<_> = text.lines().collect();
            assert!(lines.iter().all(|line| line.len() <= 76));
            assert!(lines[..lines.len() - 1].iter().all(|line| line.len() == 76));
            assert_eq!(STANDARD.decode(lines.concat()).unwrap(), bytes);
        }
    }

    #[test]
    fn png_preserves_colors_row_order_and_ignores_reserved_alpha() {
        let pixels = [0, 0, 255, 0, 0, 255, 0, 255, 255, 0, 0, 0, 255, 255, 255, 0];
        let text = encode(&pixels, 2, 2).unwrap();
        let png = STANDARD.decode(text.lines().collect::<String>()).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let mut reader = png::Decoder::new(png.as_slice()).read_info().unwrap();
        let mut output = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut output).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(info.color_type, png::ColorType::Rgb);
        assert_eq!(
            &output[..info.buffer_size()],
            &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]
        );
    }

    #[test]
    fn rejects_invalid_pixels() {
        for (width, height) in [(0, 1), (-1, 1), (1, 0), (2, 2), (i32::MAX, i32::MAX)] {
            assert!(encode(&[0; 4], width, height).is_err());
        }
    }
}
