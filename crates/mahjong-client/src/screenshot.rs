//! Native screenshot capture and PNG persistence.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use macroquad::prelude::{Image, get_screen_data};

static SCREENSHOT_SEQUENCE: AtomicU32 = AtomicU32::new(0);

/// Captures the current framebuffer and saves it under `screenshots/`.
pub fn capture() -> Result<PathBuf, String> {
    let directory = PathBuf::from("screenshots");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?;
    let sequence = SCREENSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = directory.join(screenshot_file_name(
        now.as_secs(),
        now.subsec_millis(),
        sequence,
    ));

    let mut image = get_screen_data();
    flip_vertical(&mut image);
    image::save_buffer(
        &path,
        &image.bytes,
        image.width.into(),
        image.height.into(),
        image::ColorType::Rgba8,
    )
    .map_err(|error| format!("could not write {}: {error}", path.display()))?;

    Ok(path)
}

fn screenshot_file_name(seconds: u64, milliseconds: u32, sequence: u32) -> String {
    format!("riichi-mahjong-{seconds}-{milliseconds:03}-{sequence:03}.png")
}

fn flip_vertical(image: &mut Image) {
    let row_len = image.width as usize * 4;
    for y in 0..image.height as usize / 2 {
        let opposite_y = image.height as usize - y - 1;
        for x in 0..row_len {
            image.bytes.swap(y * row_len + x, opposite_y * row_len + x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_name_contains_timestamp_and_sequence() {
        assert_eq!(
            screenshot_file_name(1_725_000_000, 7, 12),
            "riichi-mahjong-1725000000-007-012.png"
        );
    }

    #[test]
    fn captured_pixels_are_flipped_for_png_orientation() {
        let mut image = Image {
            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            width: 2,
            height: 2,
        };

        flip_vertical(&mut image);

        assert_eq!(
            image.bytes,
            vec![9, 10, 11, 12, 13, 14, 15, 16, 1, 2, 3, 4, 5, 6, 7, 8]
        );
    }
}
