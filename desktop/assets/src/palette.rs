//! Palette extraction from a wallpaper.
//!
//! The extracted palette goes through the same generator the rest of the
//! platform uses, so a wallpaper-derived theme is a normal theme: it can be
//! previewed, saved, exported and applied like any other.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use theme_engine::loader::SCHEMA_VERSION;
use theme_engine::{
    Color, LoadedTheme, Registry, ThemeColors, ThemeMetadata, generate_palette, render_from_parts,
};

use crate::AssetError;

/// Longest side of the thumbnail that is analysed.
const SAMPLE_SIDE: u32 = 96;

/// How many frequent colours are considered when picking a seed.
const CANDIDATES: usize = 8;

#[derive(Debug, Clone)]
pub struct Proposal {
    pub wallpaper: PathBuf,
    pub width: u32,
    pub height: u32,
    pub seed: Color,
    pub darkest: Color,
    /// Most frequent colours, highest count first.
    pub dominant: Vec<(Color, u32)>,
    pub palette: ThemeColors,
}

#[derive(Default, Clone, Copy)]
struct Bucket {
    count: u32,
    red: u64,
    green: u64,
    blue: u64,
}

impl Bucket {
    fn add(&mut self, red: u8, green: u8, blue: u8) {
        self.count += 1;
        self.red += u64::from(red);
        self.green += u64::from(green);
        self.blue += u64::from(blue);
    }

    fn color(self) -> Color {
        let count = u64::from(self.count.max(1));

        Color::new(
            (self.red / count) as u8,
            (self.green / count) as u8,
            (self.blue / count) as u8,
        )
    }
}

fn luminance(color: Color) -> u32 {
    u32::from(color.red) + u32::from(color.green) + u32::from(color.blue)
}

fn chroma(color: Color) -> u8 {
    let max = color.red.max(color.green).max(color.blue);
    let min = color.red.min(color.green).min(color.blue);

    max - min
}

/// Extracts a palette proposal from an image.
pub fn extract_palette(path: impl AsRef<Path>) -> Result<Proposal, AssetError> {
    let path = path.as_ref();
    let image = image::open(path)?;
    let (width, height) = (image.width(), image.height());

    if width == 0 || height == 0 {
        return Err(AssetError::Invalid(format!(
            "{} has no pixels",
            path.display()
        )));
    }

    let scale = SAMPLE_SIDE as f64 / f64::from(width.max(height));
    let sample_width = ((f64::from(width) * scale).round() as u32).max(1);
    let sample_height = ((f64::from(height) * scale).round() as u32).max(1);

    let rgb = image.to_rgb8();
    let thumbnail = image::imageops::resize(
        &rgb,
        sample_width,
        sample_height,
        image::imageops::FilterType::Triangle,
    );

    // Five bits per channel keeps similar shades in the same bucket.
    let mut buckets: BTreeMap<(u8, u8, u8), Bucket> = BTreeMap::new();

    for pixel in thumbnail.pixels() {
        let [red, green, blue] = pixel.0;

        buckets
            .entry((red >> 3, green >> 3, blue >> 3))
            .or_default()
            .add(red, green, blue);
    }

    let mut colours: Vec<(Color, u32)> = buckets
        .into_values()
        .map(|bucket| (bucket.color(), bucket.count))
        .collect();

    colours.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.to_hex().cmp(&right.0.to_hex()))
    });

    if colours.is_empty() {
        return Err(AssetError::Invalid(format!(
            "{} has no readable pixels",
            path.display()
        )));
    }

    let candidates = &colours[..colours.len().min(CANDIDATES)];

    let darkest = candidates
        .iter()
        .map(|(color, _)| *color)
        .min_by_key(|color| luminance(*color))
        .unwrap_or(colours[0].0);

    // The seed is the most chromatic frequent colour, so a grey wallpaper falls
    // back to its most common shade instead of an arbitrary hue.
    let seed = candidates
        .iter()
        .map(|(color, _)| *color)
        .max_by_key(|color| chroma(*color))
        .filter(|color| chroma(*color) >= 24)
        .unwrap_or(colours[0].0);

    let mut palette = generate_palette(seed);

    // Echo the wallpaper's darkest shade in the surfaces without losing the
    // readability guarantees of the generator.
    palette.background = palette.background.mix(darkest, 0.35);
    palette.surface = palette.surface.mix(darkest, 0.30);
    palette.surface_alt = palette.surface_alt.mix(darkest, 0.25);
    palette.border = palette.border.mix(darkest, 0.20);

    Ok(Proposal {
        wallpaper: path.to_path_buf(),
        width,
        height,
        seed,
        darkest,
        dominant: colours.into_iter().take(CANDIDATES).collect(),
        palette,
    })
}

/// Writes a generated theme into a theme root and verifies it loads back.
pub fn save_theme(
    themes_directory: impl AsRef<Path>,
    id: &str,
    name: &str,
    source: &Path,
    colors: &ThemeColors,
) -> Result<LoadedTheme, AssetError> {
    theme_engine::validate_id(id).map_err(|error| AssetError::Invalid(error.to_string()))?;

    let directory = themes_directory.as_ref().to_path_buf();
    let metadata = ThemeMetadata {
        name: name.to_string(),
        id: id.to_string(),
        version: "1.0.0".to_string(),
        description: format!("Generated from the wallpaper {}", source.display()),
        author: Some("My Linux".to_string()),
        schema: SCHEMA_VERSION,
        extends: None,
    };

    let file = directory.join(id).join("colors").join("palette.toml");

    fs::create_dir_all(
        file.parent()
            .ok_or_else(|| AssetError::Invalid("Invalid theme path".to_string()))?,
    )?;

    fs::write(&file, render_from_parts(&metadata, colors))?;

    match Registry::new(vec![directory]).load(id) {
        Ok(theme) => Ok(theme),
        Err(error) => {
            let _ = fs::remove_file(&file);

            Err(AssetError::Invalid(format!(
                "The generated theme is not valid: {error}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("my-linux-palette-{}-{}", name, std::process::id()));

        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();

        directory
    }

    fn wallpaper(directory: &Path, name: &str, colours: &[Color]) -> PathBuf {
        let path = directory.join(name);

        crate::testing::write_png(&path, 64, 32, colours).unwrap();

        path
    }

    #[test]
    fn extracts_dimensions_and_a_usable_palette() {
        let directory = scratch("extract");
        let path = wallpaper(
            &directory,
            "neon.png",
            &[Color::new(12, 8, 18), Color::new(255, 35, 90)],
        );

        let proposal = extract_palette(&path).unwrap();

        assert_eq!(proposal.width, 64);
        assert_eq!(proposal.height, 32);
        assert_eq!(proposal.wallpaper, path);
        assert!(!proposal.dominant.is_empty());
        assert_ne!(proposal.palette.primary, proposal.palette.secondary);
        assert!(
            luminance(proposal.palette.background) < luminance(proposal.palette.text),
            "a wallpaper theme must stay readable"
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn grey_wallpapers_still_produce_a_theme() {
        let directory = scratch("grey");
        let path = wallpaper(&directory, "grey.png", &[Color::new(120, 120, 120)]);

        let proposal = extract_palette(&path).unwrap();

        assert_eq!(proposal.seed, Color::new(120, 120, 120));
        assert!(proposal.palette.primary.to_hex().starts_with('#'));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_missing_and_invalid_files() {
        assert!(extract_palette("/definitely/missing/wallpaper.png").is_err());

        let directory = scratch("invalid");
        let path = directory.join("broken.png");

        fs::write(&path, "not an image").unwrap();

        assert!(extract_palette(&path).is_err());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn saves_a_theme_that_reloads() {
        let directory = scratch("save");
        let proposal = extract_palette(wallpaper(
            &directory,
            "wall.png",
            &[Color::new(20, 20, 30), Color::new(0, 230, 255)],
        ))
        .unwrap();

        let themes = directory.join("themes");
        let theme = save_theme(
            &themes,
            "wall",
            "Wall",
            &proposal.wallpaper,
            &proposal.palette,
        )
        .unwrap();

        assert_eq!(theme.metadata.id, "wall");
        assert_eq!(
            theme.colors.primary.to_hex(),
            proposal.palette.primary.to_hex()
        );
        assert!(themes.join("wall/colors/palette.toml").is_file());
        assert!(
            save_theme(
                &themes,
                "Bad Id",
                "Bad",
                &proposal.wallpaper,
                &proposal.palette
            )
            .is_err()
        );

        fs::remove_dir_all(directory).unwrap();
    }
}
