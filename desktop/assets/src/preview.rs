//! Terminal wallpaper preview using truecolor half blocks.

use std::path::Path;

use crate::AssetError;

/// Widest preview My Linux prints, in terminal cells.
const MAX_WIDTH: u32 = 80;
const MIN_WIDTH: u32 = 8;

/// Renders `path` as ANSI truecolor, one terminal row per two pixel rows.
pub fn render_preview(path: impl AsRef<Path>, width: u32) -> Result<String, AssetError> {
    let image = image::open(path)?;
    let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
    let (source_width, source_height) = (image.width(), image.height());

    if source_width == 0 || source_height == 0 {
        return Err(AssetError::Invalid("The image has no pixels".to_string()));
    }

    let scale = f64::from(width) / f64::from(source_width);
    let rows = ((f64::from(source_height) * scale) / 2.0).round().max(1.0) as u32;

    let resized = image
        .resize_exact(width, rows * 2, image::imageops::FilterType::Triangle)
        .to_rgb8();

    let mut output = String::new();

    for row in 0..rows {
        for column in 0..width {
            let top = resized.get_pixel(column, row * 2).0;
            let bottom = resized.get_pixel(column, row * 2 + 1).0;

            output.push_str(&format!(
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                top[0], top[1], top[2], bottom[0], bottom[1], bottom[2]
            ));
        }

        output.push_str("\x1b[0m\n");
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use theme_engine::Color;

    fn scratch(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("my-linux-preview-{}-{}", name, std::process::id()));

        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();

        directory
    }

    #[test]
    fn renders_truecolor_cells() {
        let directory = scratch("render");
        let path = directory.join("wall.png");

        crate::testing::write_png(&path, 32, 32, &[Color::new(255, 35, 90)]).unwrap();

        let preview = render_preview(&path, 16).unwrap();

        assert!(preview.contains("\x1b[38;2;"));
        assert!(preview.contains("\x1b[48;2;"));
        // A 32x32 image shown 16 cells wide uses two pixel rows per cell.
        assert_eq!(preview.lines().count(), 8);

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn clamps_the_requested_width() {
        let directory = scratch("clamp");
        let path = directory.join("wall.png");

        crate::testing::write_png(&path, 400, 200, &[Color::new(10, 10, 10)]).unwrap();

        let preview = render_preview(&path, 400).unwrap();
        let cells = preview.lines().next().unwrap().matches('\u{2580}').count();

        assert_eq!(cells, MAX_WIDTH as usize);

        std::fs::remove_dir_all(directory).unwrap();
    }
}
