use anyhow::anyhow;
use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

/// Embedded asset source for Fronda icons.
///
/// Assets are compiled into the binary via `include_bytes!` so no runtime path resolution is needed.
pub struct FrondaAssets;

impl AssetSource for FrondaAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match path {
            "icons/chat.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/chat.svg")))),
            "icons/export.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/export.svg")))),
            "icons/undo.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/undo.svg")))),
            "icons/redo.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/redo.svg")))),
            "icons/cursor.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/cursor.svg")))),
            "icons/razor.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/razor.svg")))),
            "icons/split.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/split.svg")))),
            "icons/keyboard.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/keyboard.svg")))),
            "icons/network.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/network.svg")))),
            "icons/plus.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/plus.svg")))),
            "icons/folder.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/folder.svg")))),
            "icons/gear.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/gear.svg")))),
            "icons/play.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/play.svg")))),
            "icons/pause.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/pause.svg")))),
            "icons/skip_back.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/skip_back.svg")))),
            "icons/skip_forward.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/skip_forward.svg")))),
            "icons/step_back.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/step_back.svg")))),
            "icons/step_forward.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/step_forward.svg")))),
            "icons/camera.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/camera.svg")))),
            "icons/video.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/video.svg")))),
            "icons/photo.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/photo.svg")))),
            "icons/waveform.svg" => Ok(Some(Cow::Borrowed(include_bytes!("../assets/icons/waveform.svg")))),
            _ => Err(anyhow!("unknown asset: {path}")),
        }
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
