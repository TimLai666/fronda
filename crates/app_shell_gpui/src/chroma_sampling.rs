//! Cross-view hand-off for the chroma-key eyedropper (upstream #291): the
//! inspector arms it with the target clip id when the eyedropper button is
//! pressed; the preview consumes it on the next canvas click, samples the pixel
//! there, and applies `key.chroma` with the sampled hue.

use std::sync::Mutex;

static SAMPLING: Mutex<Option<String>> = Mutex::new(None);

/// Arm (or clear) sampling for a clip id.
pub fn set_sampling(clip_id: Option<String>) {
    if let Ok(mut g) = SAMPLING.lock() {
        *g = clip_id;
    }
}

/// The clip currently awaiting a sample, without consuming it (for UI state).
pub fn sampling_clip() -> Option<String> {
    SAMPLING.lock().ok().and_then(|g| g.clone())
}

/// Consume the armed clip id (the preview calls this on click).
pub fn take_sampling() -> Option<String> {
    SAMPLING.lock().ok().and_then(|mut g| g.take())
}
