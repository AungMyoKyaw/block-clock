use gpui::{Rgba, WindowAppearance, rgb};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Palette {
    pub bg: Rgba,
    pub surface: Rgba,
    pub future: Rgba,
    pub grid: Rgba,
    pub grid_strong: Rgba,
    pub past_day: Rgba,
    pub current_hour_past: Rgba,
    pub now: Rgba,
    pub now_ring: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub text_faint: Rgba,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            bg: rgb(0xECEEEB),
            surface: rgb(0xFAFBF8),
            future: rgb(0xFFFFFF),
            grid: rgb(0xD7DCDE),
            grid_strong: rgb(0xB9C2C5),
            past_day: rgb(0xC5CED3),
            current_hour_past: rgb(0x809CB0),
            now: rgb(0xC2410C),
            now_ring: rgb(0xA95500),
            text: rgb(0x16191C),
            text_muted: rgb(0x5E6970),
            text_faint: rgb(0x4F5860),
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: rgb(0x0B0E12),
            surface: rgb(0x0F1419),
            future: rgb(0x151A20),
            grid: rgb(0x29313A),
            grid_strong: rgb(0x424D58),
            past_day: rgb(0x3C4854),
            current_hour_past: rgb(0x66839A),
            now: rgb(0xFB923C),
            now_ring: rgb(0xF5B942),
            text: rgb(0xF4F7F9),
            text_muted: rgb(0x909CA8),
            text_faint: rgb(0x7D8996),
        }
    }

    pub fn resolve(mode: ThemeMode, appearance: WindowAppearance) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::System => match appearance {
                WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
                WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_theme_and_system_appearance() {
        assert_eq!(
            Palette::resolve(ThemeMode::Light, WindowAppearance::Dark),
            Palette::light()
        );
        assert_eq!(
            Palette::resolve(ThemeMode::Dark, WindowAppearance::Light),
            Palette::dark()
        );
        assert_eq!(
            Palette::resolve(ThemeMode::System, WindowAppearance::Light),
            Palette::light()
        );
        assert_eq!(
            Palette::resolve(ThemeMode::System, WindowAppearance::VibrantLight),
            Palette::light()
        );
        assert_eq!(
            Palette::resolve(ThemeMode::System, WindowAppearance::Dark),
            Palette::dark()
        );
        assert_eq!(
            Palette::resolve(ThemeMode::System, WindowAppearance::VibrantDark),
            Palette::dark()
        );
    }
}
