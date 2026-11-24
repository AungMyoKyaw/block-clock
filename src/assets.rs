use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

pub struct Assets;

const PIN_SVG: &[u8] = include_bytes!("../assets/icons/pin.svg");
const MAXIMIZE_SVG: &[u8] = include_bytes!("../assets/icons/maximize.svg");
const MINIMIZE_SVG: &[u8] = include_bytes!("../assets/icons/minimize.svg");
const SETTINGS_SVG: &[u8] = include_bytes!("../assets/icons/settings.svg");
const CLOSE_SVG: &[u8] = include_bytes!("../assets/icons/close.svg");

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes = match path {
            "icons/pin.svg" => Some(PIN_SVG),
            "icons/maximize.svg" => Some(MAXIMIZE_SVG),
            "icons/minimize.svg" => Some(MINIMIZE_SVG),
            "icons/settings.svg" => Some(SETTINGS_SVG),
            "icons/close.svg" => Some(CLOSE_SVG),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        if path.is_empty() || path == "icons" {
            Ok(vec![
                "icons/pin.svg".into(),
                "icons/maximize.svg".into(),
                "icons/minimize.svg".into(),
                "icons/settings.svg".into(),
                "icons/close.svg".into(),
            ])
        } else {
            Ok(Vec::new())
        }
    }
}
