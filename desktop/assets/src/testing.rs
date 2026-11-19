//! Test helpers.
//!
//! These are not used by the platform itself; they let tests create wallpaper
//! images without checking binaries into the repository.

use std::path::Path;

use theme_engine::Color;

use crate::AssetError;

/// Writes a PNG made of 8x8 blocks cycling through `colours`.
#[doc(hidden)]
pub fn write_png(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    colours: &[Color],
) -> Result<(), AssetError> {
    let mut buffer = image::RgbImage::new(width, height);
    let count = colours.len();

    for (x, y, pixel) in buffer.enumerate_pixels_mut() {
        let colour = if count == 0 {
            Color::new(0, 0, 0)
        } else {
            colours[((x / 8 + y / 8) as usize) % count]
        };

        *pixel = image::Rgb([colour.red, colour.green, colour.blue]);
    }

    buffer.save(path)?;

    Ok(())
}
