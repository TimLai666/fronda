use serde::{Deserialize, Serialize};

/// One entry in a clip's ordered effect stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    #[serde(default = "new_effect_id")]
    pub id: String,
    pub r#type: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub params: std::collections::HashMap<String, EffectParam>,
}

fn new_effect_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_enabled() -> bool {
    true
}

impl Effect {
    /// Convenience constructor for static numeric params (e.g. for tests).
    pub fn new(type_name: &str, params: Vec<(&str, f64)>) -> Self {
        Self {
            id: new_effect_id(),
            r#type: type_name.to_string(),
            enabled: true,
            params: params
                .into_iter()
                .map(|(k, v)| (k.to_string(), EffectParam::value(v)))
                .collect(),
        }
    }
}

/// A single effect parameter — numeric value, string value, or keyframed track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectParam {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<super::KeyframeTrack<f64>>,
}

impl EffectParam {
    pub fn value(v: f64) -> Self {
        Self {
            value: Some(v),
            string: None,
            track: None,
        }
    }

    pub fn string(s: &str) -> Self {
        Self {
            value: None,
            string: Some(s.to_string()),
            track: None,
        }
    }

    /// Effective numeric value at a clip-relative frame offset.
    /// Returns `default` when both value and track are absent.
    pub fn resolved_at(&self, offset: i64, default: f64) -> f64 {
        if let Some(ref track) = self.track {
            if !track.keyframes.is_empty() {
                return sample_keyframe_track(track, offset, self.value.unwrap_or(default));
            }
        }
        self.value.unwrap_or(default)
    }
}

fn sample_keyframe_track(track: &super::KeyframeTrack<f64>, offset: i64, fallback: f64) -> f64 {
    let kfs = &track.keyframes;
    if kfs.is_empty() {
        return fallback;
    }
    if offset <= kfs[0].frame {
        return kfs[0].value;
    }
    if offset >= kfs[kfs.len() - 1].frame {
        return kfs[kfs.len() - 1].value;
    }
    for i in 1..kfs.len() {
        if offset <= kfs[i].frame {
            let prev = &kfs[i - 1];
            let next = &kfs[i];
            if next.frame == prev.frame {
                return next.value;
            }
            let t = (offset - prev.frame) as f64 / (next.frame - prev.frame) as f64;
            return match prev.interpolation_out {
                super::Interpolation::Hold => prev.value,
                super::Interpolation::Linear => prev.value + (next.value - prev.value) * t,
                super::Interpolation::Smooth => {
                    // Smoothstep
                    let st = t * t * (3.0 - 2.0 * t);
                    prev.value + (next.value - prev.value) * st
                }
            };
        }
    }
    fallback
}

/// A control point on a tone curve (input → output, both 0–1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub x: f64,
    pub y: f64,
}

/// Master (luma) + per-channel R/G/B tone curves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeCurve {
    #[serde(default)]
    pub master: Vec<CurvePoint>,
    #[serde(default)]
    pub red: Vec<CurvePoint>,
    #[serde(default)]
    pub green: Vec<CurvePoint>,
    #[serde(default)]
    pub blue: Vec<CurvePoint>,
}

impl Default for GradeCurve {
    fn default() -> Self {
        Self {
            master: Vec::new(),
            red: Vec::new(),
            green: Vec::new(),
            blue: Vec::new(),
        }
    }
}

impl GradeCurve {
    pub const IDENTITY: [CurvePoint; 2] =
        [CurvePoint { x: 0.0, y: 0.0 }, CurvePoint { x: 1.0, y: 1.0 }];

    pub fn is_identity(&self) -> bool {
        self.master.is_empty()
            && self.red.is_empty()
            && self.green.is_empty()
            && self.blue.is_empty()
    }

    /// Piecewise-linear interpolation, clamped outside range.
    pub fn eval(points: &[CurvePoint], x: f64) -> f64 {
        let mut pts: Vec<CurvePoint> = if points.is_empty() {
            Self::IDENTITY.to_vec()
        } else {
            points.to_vec()
        };
        pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        if x <= pts[0].x {
            return pts[0].y;
        }
        if x >= pts[pts.len() - 1].x {
            return pts[pts.len() - 1].y;
        }
        for i in 1..pts.len() {
            if x <= pts[i].x {
                let a = pts[i - 1];
                let b = pts[i];
                let t = if (b.x - a.x).abs() < 1e-10 {
                    0.0
                } else {
                    (x - a.x) / (b.x - a.x)
                };
                return a.y + (b.y - a.y) * t;
            }
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_serde_round_trip() {
        let e = Effect::new("color.exposure", vec![("ev", 0.5)]);
        let json = serde_json::to_string(&e).unwrap();
        let decoded: Effect = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.r#type, "color.exposure");
        assert_eq!(decoded.params["ev"].value, Some(0.5));
    }

    #[test]
    fn effect_param_value_round_trip() {
        let p = EffectParam::value(0.75);
        let json = serde_json::to_string(&p).unwrap();
        let decoded: EffectParam = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.value, Some(0.75));
    }

    #[test]
    fn effect_param_string_round_trip() {
        let p = EffectParam::string("/path/to/lut.cube");
        let json = serde_json::to_string(&p).unwrap();
        let decoded: EffectParam = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.string.as_deref(), Some("/path/to/lut.cube"));
    }

    #[test]
    fn effect_param_resolved_value() {
        let p = EffectParam::value(0.5);
        assert!((p.resolved_at(0, 1.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn effect_param_resolved_fallback() {
        let p = EffectParam {
            value: None,
            string: None,
            track: None,
        };
        assert!((p.resolved_at(0, 1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn grade_curve_identity() {
        let curve = GradeCurve::default();
        assert!(curve.is_identity());
    }

    #[test]
    fn grade_curve_eval_identity() {
        let curve = GradeCurve::default();
        let result = GradeCurve::eval(&curve.master, 0.5);
        assert!((result - 0.5).abs() < 1e-10);
    }

    #[test]
    fn grade_curve_eval_custom() {
        let pts = vec![
            CurvePoint { x: 0.0, y: 0.06 },
            CurvePoint { x: 1.0, y: 0.95 },
        ];
        let result = GradeCurve::eval(&pts, 0.5);
        assert!((result - 0.505).abs() < 1e-10);
    }
}
