//! Palette generation from a single seed colour.

use crate::Color;
use crate::loader::ThemeColors;

fn rgb_to_hsl(color: Color) -> (f64, f64, f64) {
    let red = f64::from(color.red) / 255.0;
    let green = f64::from(color.green) / 255.0;
    let blue = f64::from(color.blue) / 255.0;

    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, lightness);
    }

    let delta = max - min;

    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let hue = if (max - red).abs() < f64::EPSILON {
        ((green - blue) / delta) % 6.0
    } else if (max - green).abs() < f64::EPSILON {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    };

    ((hue * 60.0 + 360.0) % 360.0, saturation, lightness)
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> Color {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = (hue / 60.0).rem_euclid(6.0);
    let second = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());

    let (red, green, blue) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };

    let offset = lightness - chroma / 2.0;

    let channel = |value: f64| ((value + offset).clamp(0.0, 1.0) * 255.0).round() as u8;

    Color::new(channel(red), channel(green), channel(blue))
}

/// Builds a complete, readable dark palette from `base`.
///
/// The seed supplies the hue; saturation and lightness are normalised so that
/// unusual seeds still produce a usable desktop theme. The result is
/// deterministic, which makes generated themes reproducible.
pub fn generate_palette(base: Color) -> ThemeColors {
    let (hue, saturation, _) = rgb_to_hsl(base);

    let saturation = saturation.clamp(0.45, 0.95);
    let accent = hsl_to_rgb(hue, saturation, 0.62);
    let secondary = hsl_to_rgb((hue + 150.0).rem_euclid(360.0), saturation, 0.60);

    ThemeColors {
        background: hsl_to_rgb(hue, saturation * 0.45, 0.08),
        surface: hsl_to_rgb(hue, saturation * 0.40, 0.14),
        surface_alt: hsl_to_rgb(hue, saturation * 0.40, 0.20),
        primary: accent,
        secondary,
        text: hsl_to_rgb(hue, 0.16, 0.96),
        text_muted: hsl_to_rgb(hue, 0.16, 0.72),
        border: hsl_to_rgb(hue, saturation * 0.40, 0.32),
        success: hsl_to_rgb(145.0, 0.65, 0.55),
        warning: hsl_to_rgb(45.0, 0.90, 0.60),
        error: hsl_to_rgb(0.0, 0.75, 0.60),
        active: accent,
        hover: secondary,
        selected: hsl_to_rgb(hue, saturation * 0.45, 0.24),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_stable_palette() {
        let first = generate_palette(Color::new(255, 35, 90));
        let second = generate_palette(Color::new(255, 35, 90));

        assert_eq!(first.background.to_hex(), second.background.to_hex());
        assert_eq!(first.primary.to_hex(), second.primary.to_hex());
    }

    #[test]
    fn generated_palette_has_contrast_and_distinct_accents() {
        let palette = generate_palette(Color::new(110, 190, 90));

        assert_ne!(palette.background, palette.primary);
        assert_ne!(palette.primary, palette.secondary);
        assert_ne!(palette.text, palette.background);

        // Dark theme: the background must be clearly darker than the text.
        let luminance =
            |color: Color| u32::from(color.red) + u32::from(color.green) + u32::from(color.blue);

        assert!(luminance(palette.background) < luminance(palette.text));
    }

    #[test]
    fn different_seeds_produce_different_hues() {
        let red = generate_palette(Color::new(255, 35, 90));
        let green = generate_palette(Color::new(110, 190, 90));

        assert_ne!(red.primary.to_hex(), green.primary.to_hex());
    }

    #[test]
    fn converts_between_rgb_and_hsl() {
        let (hue, saturation, lightness) = rgb_to_hsl(Color::new(255, 0, 0));

        assert!((hue - 0.0).abs() < 0.001);
        assert!((saturation - 1.0).abs() < 0.001);
        assert!((lightness - 0.5).abs() < 0.001);

        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), Color::new(255, 0, 0));
    }
}
