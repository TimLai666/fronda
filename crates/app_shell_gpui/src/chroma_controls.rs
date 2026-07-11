//! Pure logic for the Chroma Key inspector section + preview eyedropper
//! (upstream #291). Reads/writes the `key.chroma` effect the compositor already
//! renders as a soft hue key. No gpui here — the view builds `apply_effect`
//! args from these and applies through the shared executor.

use core_model::ChromaKey;

/// RGB (0..1) → hue (0..1), matching the compositor's key-hue convention.
pub fn rgb_to_hue(r: f64, g: f64, b: f64) -> f64 {
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let dd = mx - mn;
    if dd <= 1e-5 {
        0.0
    } else if mx == r {
        (((g - b) / dd) / 6.0).rem_euclid(1.0)
    } else if mx == g {
        (((b - r) / dd + 2.0) / 6.0).rem_euclid(1.0)
    } else {
        (((r - g) / dd + 4.0) / 6.0).rem_euclid(1.0)
    }
}

/// Hue (0..1) → full saturation/value RGB (0..1), for the key-colour swatch.
pub fn hue_to_rgb(hue: f64) -> (f64, f64, f64) {
    let h = hue.rem_euclid(1.0) * 6.0;
    let x = 1.0 - ((h % 2.0) - 1.0).abs();
    match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    }
}

/// Chroma-key control values shown in the inspector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromaControls {
    pub enabled: bool,
    pub key_r: f64,
    pub key_g: f64,
    pub key_b: f64,
    pub tolerance: f64,
    pub softness: f64,
    pub spill: f64,
}

impl Default for ChromaControls {
    fn default() -> Self {
        // Swift eyedropper defaults over a green-screen key.
        Self {
            enabled: false,
            key_r: 0.0,
            key_g: 1.0,
            key_b: 0.0,
            tolerance: 0.15,
            softness: 0.1,
            spill: 0.5,
        }
    }
}

impl ChromaControls {
    /// Read the selected clip's stored key (or defaults when unset).
    pub fn from_chroma_key(key: Option<&ChromaKey>) -> Self {
        match key {
            Some(k) => Self {
                enabled: k.enabled,
                key_r: k.key_r,
                key_g: k.key_g,
                key_b: k.key_b,
                tolerance: k.tolerance,
                softness: k.softness,
                spill: k.spill_suppression,
            },
            None => Self::default(),
        }
    }

    pub fn key_hue(&self) -> f64 {
        rgb_to_hue(self.key_r, self.key_g, self.key_b)
    }

    /// Replace the key colour from a sampled hue (eyedropper / preset).
    pub fn with_hue(mut self, hue: f64) -> Self {
        let (r, g, b) = hue_to_rgb(hue);
        self.key_r = r;
        self.key_g = g;
        self.key_b = b;
        self
    }

    /// `apply_effect` args (v2 batch shape) for these controls over `clip_ids`.
    pub fn apply_args(&self, clip_ids: &[String]) -> serde_json::Value {
        serde_json::json!({
            "clipIds": clip_ids,
            "effects": [{
                "type": "key.chroma",
                "enabled": self.enabled,
                "params": {
                    "keyHue": self.key_hue(),
                    "tolerance": self.tolerance,
                    "softness": self.softness,
                    "spill": self.spill,
                }
            }]
        })
    }
}

/// Map a canvas-relative click (px) onto normalized frame coords (u,v ∈ 0..1)
/// for an aspect-fit frame of `frame_aspect` (w/h) inside a `canvas` (w,h) box.
/// Returns `None` when the click lands in the letterbox / pillarbox bars.
pub fn frame_uv_from_click(
    click: (f32, f32),
    canvas: (f32, f32),
    frame_aspect: f64,
) -> Option<(f64, f64)> {
    let (cw, ch) = (canvas.0 as f64, canvas.1 as f64);
    if cw <= 0.0 || ch <= 0.0 || frame_aspect <= 0.0 {
        return None;
    }
    let canvas_aspect = cw / ch;
    let (fw, fh) = if frame_aspect >= canvas_aspect {
        (cw, cw / frame_aspect) // wider than canvas → full width, letterbox top/bottom
    } else {
        (ch * frame_aspect, ch) // taller → full height, pillarbox left/right
    };
    let ox = (cw - fw) / 2.0;
    let oy = (ch - fh) / 2.0;
    let u = (click.0 as f64 - ox) / fw;
    let v = (click.1 as f64 - oy) / fh;
    if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
        Some((u, v))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn rgb_to_hue_primaries() {
        assert!(approx(rgb_to_hue(1.0, 0.0, 0.0), 0.0), "red");
        assert!(approx(rgb_to_hue(0.0, 1.0, 0.0), 1.0 / 3.0), "green");
        assert!(approx(rgb_to_hue(0.0, 0.0, 1.0), 2.0 / 3.0), "blue");
        assert!(approx(rgb_to_hue(0.5, 0.5, 0.5), 0.0), "grey → 0");
    }

    #[test]
    fn hue_rgb_round_trips() {
        for &h in &[0.0, 1.0 / 3.0, 0.5, 2.0 / 3.0, 0.85] {
            let (r, g, b) = hue_to_rgb(h);
            assert!(approx(rgb_to_hue(r, g, b), h), "round-trip hue {h}");
        }
    }

    #[test]
    fn controls_default_and_from_key() {
        let d = ChromaControls::default();
        assert!(!d.enabled);
        assert!(approx(d.key_hue(), 1.0 / 3.0), "default green key");

        let key = ChromaKey {
            enabled: true,
            key_r: 0.0,
            key_g: 0.0,
            key_b: 1.0,
            tolerance: 0.3,
            softness: 0.2,
            spill_suppression: 0.6,
        };
        let c = ChromaControls::from_chroma_key(Some(&key));
        assert!(c.enabled);
        assert!(approx(c.key_hue(), 2.0 / 3.0), "blue key");
        assert!(approx(c.spill, 0.6));
    }

    #[test]
    fn apply_args_shape() {
        let c = ChromaControls::default().with_hue(2.0 / 3.0); // blue
        let args = c.apply_args(&["clip-1".into()]);
        assert_eq!(args["clipIds"][0], "clip-1");
        assert_eq!(args["effects"][0]["type"], "key.chroma");
        let hue = args["effects"][0]["params"]["keyHue"].as_f64().unwrap();
        assert!(approx(hue, 2.0 / 3.0), "keyHue carries the sampled hue");
        assert_eq!(args["effects"][0]["params"]["tolerance"], 0.15);
    }

    #[test]
    fn eyedropper_maps_center_and_rejects_letterbox() {
        // 16:9 frame in a square 100×100 canvas → letterbox top/bottom.
        let aspect = 16.0 / 9.0;
        // Centre maps to (0.5, 0.5).
        let (u, v) = frame_uv_from_click((50.0, 50.0), (100.0, 100.0), aspect).unwrap();
        assert!(
            approx(u, 0.5) && approx(v, 0.5),
            "centre → 0.5,0.5 got {u},{v}"
        );
        // Top edge (y=0) is inside the letterbox bar → None.
        assert!(frame_uv_from_click((50.0, 0.0), (100.0, 100.0), aspect).is_none());
        // Frame fills width; its top is at y = (100 - 56.25)/2 ≈ 21.9.
        let (_, v2) = frame_uv_from_click((50.0, 22.0), (100.0, 100.0), aspect).unwrap();
        assert!(v2 < 0.05, "just inside top edge → v≈0, got {v2}");
    }

    #[test]
    fn eyedropper_square_frame_pillarbox() {
        // 1:1 frame in a 200×100 canvas → pillarbox left/right, frame 100 wide centred.
        let (u, v) = frame_uv_from_click((100.0, 50.0), (200.0, 100.0), 1.0).unwrap();
        assert!(approx(u, 0.5) && approx(v, 0.5));
        assert!(
            frame_uv_from_click((10.0, 50.0), (200.0, 100.0), 1.0).is_none(),
            "left bar"
        );
    }
}
