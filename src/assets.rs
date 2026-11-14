use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// All artwork is embedded into the binary so the app is a single self-contained exe.
pub struct Assets;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[
            $((concat!("icons/", $name, ".svg"), include_bytes!(concat!("../assets/icons/", $name, ".svg"))),)*
        ];
    };
}

icons!(
    "archive",
    "arrow_left",
    "bolt",
    "check",
    "chevron_down",
    "chevron_right",
    "clock",
    "close",
    "cloud",
    "compose",
    "download",
    "feather",
    "file",
    "folder",
    "forward",
    "inbox",
    "layers",
    "languages",
    "lock",
    "mail",
    "maximize",
    "minimize",
    "moon",
    "more",
    "paperclip",
    "plus",
    "refresh",
    "reply",
    "reply_all",
    "search",
    "send",
    "server",
    "settings",
    "sparkles",
    "star",
    "sun",
    "tag",
    "trash",
    "user",
);

const LOGO: &[u8] = include_bytes!("../assets/logo.png");

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == "logo.png" {
            return Ok(Some(Cow::Borrowed(LOGO)));
        }
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}



