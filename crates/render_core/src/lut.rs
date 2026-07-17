//! `.cube` 3D LUT parser + trilinear sampler (upstream #296 / decision D5).
//!
//! Parse mirrors Swift `LUTLoader.parse` exactly: comment/blank/TITLE tolerance,
//! case-insensitive keywords, `LUT_3D_SIZE` 2..=128 (#296 raised the cap from 64),
//! `DOMAIN_MIN`/`DOMAIN_MAX` normalization baked into the stored values (clamped
//! 0..=1), red-fastest data order, 1D LUTs rejected. Sampling clamps the input to
//! 0..=1 and interpolates trilinearly over the 8 neighbouring nodes (the design's
//! chosen scheme; Swift's Metal kernel uses tetrahedral — exact-equal at lattice
//! points and for lattice-linear LUTs such as identity).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// A parsed 3D LUT: `dimension`³ RGB nodes, red fastest, values normalized 0..=1.
#[derive(Debug, Clone, PartialEq)]
pub struct CubeLut {
    pub dimension: usize,
    pub data: Vec<f32>,
}

impl CubeLut {
    // max/min chains (not .clamp) so NaN maps to 0.0 like Swift's min(1, max(0, x)).
    #[allow(clippy::manual_clamp)]
    pub fn parse(text: &str) -> Result<CubeLut, String> {
        let mut dimension: usize = 0;
        let mut saw_size = false;
        let mut domain_min: Vec<f32> = vec![0.0, 0.0, 0.0];
        let mut domain_max: Vec<f32> = vec![1.0, 1.0, 1.0];
        let mut values: Vec<f32> = Vec::new();

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            let Some(first) = parts.first() else { continue };
            match first.to_ascii_uppercase().as_str() {
                "TITLE" => {}
                "LUT_1D_SIZE" => return Err("1D LUTs are not supported".to_string()),
                "LUT_3D_SIZE" => {
                    saw_size = true;
                    dimension = parts
                        .last()
                        .and_then(|t| t.parse::<usize>().ok())
                        .ok_or_else(|| format!("invalid LUT_3D_SIZE line: {line}"))?;
                }
                "DOMAIN_MIN" => {
                    domain_min = parts[1..].iter().filter_map(|t| t.parse().ok()).collect();
                }
                "DOMAIN_MAX" => {
                    domain_max = parts[1..].iter().filter_map(|t| t.parse().ok()).collect();
                }
                _ => {
                    // Swift skips short lines (unknown metadata) but hard-rejects a
                    // >=3-token row whose leading values aren't numbers.
                    if parts.len() < 3 {
                        continue;
                    }
                    for t in &parts[..3] {
                        let v: f32 = t
                            .parse()
                            .map_err(|_| format!("invalid value '{t}' in data row: {line}"))?;
                        values.push(v);
                    }
                }
            }
        }

        if !saw_size {
            return Err("missing LUT_3D_SIZE".to_string());
        }
        if dimension <= 1 || dimension > 128 {
            return Err(format!("LUT_3D_SIZE must be 2..=128, got {dimension}"));
        }
        let expected = dimension * dimension * dimension * 3;
        if values.len() != expected {
            return Err(format!(
                "expected {} data rows, found {}",
                expected / 3,
                values.len() / 3
            ));
        }
        if domain_min.len() != 3 {
            return Err("DOMAIN_MIN must have 3 values".to_string());
        }
        if domain_max.len() != 3 {
            return Err("DOMAIN_MAX must have 3 values".to_string());
        }

        let mut data = Vec::with_capacity(values.len());
        for chunk in values.chunks_exact(3) {
            for c in 0..3 {
                let span = (domain_max[c] - domain_min[c]).max(0.0001);
                data.push(((chunk[c] - domain_min[c]) / span).max(0.0).min(1.0));
            }
        }
        Ok(CubeLut { dimension, data })
    }

    /// Look up `rgb` (each clamped to 0..=1) with trilinear interpolation.
    #[allow(clippy::manual_clamp)] // NaN input maps to 0.0, not a NaN lattice index
    pub fn sample(&self, rgb: [f64; 3]) -> [f64; 3] {
        let n = self.dimension;
        let nf = (n - 1) as f64;
        let mut base = [0usize; 3];
        let mut frac = [0f64; 3];
        for c in 0..3 {
            let p = rgb[c].max(0.0).min(1.0) * nf;
            let b = (p.floor() as usize).min(n - 2);
            base[c] = b;
            frac[c] = p - b as f64;
        }
        let mut out = [0.0f64; 3];
        for corner in 0..8usize {
            let (dr, dg, db) = (corner & 1, (corner >> 1) & 1, (corner >> 2) & 1);
            let w = (if dr == 1 { frac[0] } else { 1.0 - frac[0] })
                * (if dg == 1 { frac[1] } else { 1.0 - frac[1] })
                * (if db == 1 { frac[2] } else { 1.0 - frac[2] });
            if w == 0.0 {
                continue;
            }
            let i = (((base[2] + db) * n + base[1] + dg) * n + base[0] + dr) * 3;
            for c in 0..3 {
                out[c] += w * self.data[i + c] as f64;
            }
        }
        out
    }
}

type LutCache = HashMap<String, Arc<CubeLut>>;
static CACHE: OnceLock<Mutex<LutCache>> = OnceLock::new();

/// Read + parse a `.cube` file, memory-cached by path (upstream #343:
/// no per-load stat — an edited LUT refreshes on restart, first cached
/// value wins over concurrent re-reads).
pub fn load_cached(path: &str) -> Result<Arc<CubeLut>, String> {
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(lut) = cache.lock().unwrap().get(path) {
        return Ok(Arc::clone(lut));
    }
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read LUT {path}: {e}"))?;
    let lut = Arc::new(CubeLut::parse(&text).map_err(|e| format!("invalid LUT {path}: {e}"))?);
    let mut guard = cache.lock().unwrap();
    if let Some(existing) = guard.get(path) {
        return Ok(Arc::clone(existing));
    }
    guard.insert(path.to_string(), Arc::clone(&lut));
    Ok(lut)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream #296 `LUTLoaderTests.cubeText`: an identity ramp cube.
    fn cube_text(n: usize) -> String {
        let mut lines = vec![format!("LUT_3D_SIZE {n}")];
        let nf = (n - 1) as f64;
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    lines.push(format!("{} {} {}", r as f64 / nf, g as f64 / nf, b as f64 / nf));
                }
            }
        }
        lines.join("\n")
    }

    // === Upstream #296 transplanted vectors ===

    #[test]
    fn parses_33_point_cube() {
        let lut = CubeLut::parse(&cube_text(33)).expect("33-point cube parses");
        assert_eq!(lut.dimension, 33);
    }

    #[test]
    fn parses_65_point_cube() {
        let lut = CubeLut::parse(&cube_text(65)).expect("65-point cube parses (#296)");
        assert_eq!(lut.dimension, 65);
        assert_eq!(lut.data.len(), 65 * 65 * 65 * 3);
    }

    #[test]
    fn rejects_129_point_cube() {
        let err = CubeLut::parse("LUT_3D_SIZE 129\n0 0 0").unwrap_err();
        assert!(err.contains("128"), "cap error names the limit: {err}");
    }

    #[test]
    fn accepts_128_size_gate() {
        // 128 passes the range gate (#296 cap): with too few rows the error is
        // about row count, not the size cap.
        let err = CubeLut::parse("LUT_3D_SIZE 128\n0 0 0").unwrap_err();
        assert!(err.contains("rows"), "row-count error, not cap: {err}");
    }

    #[test]
    fn rejects_1d_lut() {
        assert!(CubeLut::parse("LUT_1D_SIZE 16\n0 0 0").is_err());
    }

    // === Parse tolerance / rejection boundary (Swift LUTLoader.parse parity) ===

    #[test]
    fn rejects_dimension_1() {
        assert!(CubeLut::parse("LUT_3D_SIZE 1\n0 0 0").is_err());
    }

    #[test]
    fn rejects_missing_size() {
        assert!(CubeLut::parse("0 0 0\n1 1 1").is_err());
    }

    #[test]
    fn tolerates_comments_blank_title_and_case() {
        let text = format!(
            "# a comment\n\nTITLE \"My Grade\"\nlut_3d_size 2\nLUT_IN_VIDEO_RANGE\n{}",
            cube_text(2).lines().skip(1).collect::<Vec<_>>().join("\n")
        );
        let lut = CubeLut::parse(&text).expect("tolerant parse");
        assert_eq!(lut.dimension, 2);
    }

    #[test]
    fn last_size_declaration_wins() {
        let text = format!(
            "LUT_3D_SIZE 33\n{}",
            cube_text(2) // includes its own LUT_3D_SIZE 2 line
        );
        let lut = CubeLut::parse(&text).expect("last LUT_3D_SIZE wins");
        assert_eq!(lut.dimension, 2);
    }

    #[test]
    fn rejects_wrong_row_count() {
        let mut text = cube_text(2);
        text.push_str("\n0.5 0.5 0.5"); // 9 rows for a 2-cube (needs 8)
        assert!(CubeLut::parse(&text).is_err());
    }

    #[test]
    fn rejects_bad_value_in_data_row() {
        let text = "LUT_3D_SIZE 2\n0 0 x\n".to_string() + &cube_text(2);
        assert!(CubeLut::parse(&text).is_err());
    }

    #[test]
    fn rejects_three_token_metadata_line() {
        // Swift parity: a >=3-token non-keyword line must be numeric.
        let text = format!("LUT_1D_INPUT_RANGE 0.0 1.0\n{}", cube_text(2));
        assert!(CubeLut::parse(&text).is_err());
    }

    #[test]
    fn rejects_bad_domain_count() {
        let text = format!("DOMAIN_MIN 0 0\n{}", cube_text(2));
        assert!(CubeLut::parse(&text).is_err());
    }

    #[test]
    fn domain_normalizes_stored_values() {
        // Values span 0..2 with DOMAIN_MAX 2 → stored halved (Swift bakes the
        // domain into the table at parse time).
        let text = "LUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2\n\
                    0 0 0\n2 0 0\n0 2 0\n2 2 0\n0 0 2\n2 0 2\n0 2 2\n2 2 2";
        let lut = CubeLut::parse(text).expect("domain parse");
        let out = lut.sample([1.0, 1.0, 1.0]);
        for c in out {
            assert!((c - 1.0).abs() < 1e-6, "normalized to 1.0, got {c}");
        }
        let mid = lut.sample([1.0, 0.0, 0.0]);
        assert!((mid[0] - 1.0).abs() < 1e-6, "r node normalized: {mid:?}");
        assert!(mid[1].abs() < 1e-6 && mid[2].abs() < 1e-6, "{mid:?}");
    }

    #[test]
    fn out_of_domain_values_clamped() {
        let text = "LUT_3D_SIZE 2\n-1 0 0\n2 0 0\n0 2 0\n2 2 0\n0 0 2\n2 0 2\n0 2 2\n2 2 2";
        let lut = CubeLut::parse(text).expect("clamping parse");
        assert!(lut.data.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    // === Sampling ===

    #[test]
    fn identity_lut_samples_identity() {
        let lut = CubeLut::parse(&cube_text(5)).unwrap();
        for probe in [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.3, 0.7, 0.15], [0.5, 0.25, 0.99]] {
            let out = lut.sample(probe);
            for c in 0..3 {
                assert!(
                    (out[c] - probe[c]).abs() < 1e-6,
                    "identity: {probe:?} -> {out:?}"
                );
            }
        }
    }

    #[test]
    fn channel_swap_lut_swaps_exactly() {
        // Node (r,g,b) -> (g, r, b): lattice-linear, so interpolation is exact.
        let mut lines = vec!["LUT_3D_SIZE 2".to_string()];
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    lines.push(format!("{g} {r} {b}"));
                }
            }
        }
        let lut = CubeLut::parse(&lines.join("\n")).unwrap();
        let out = lut.sample([0.2, 0.8, 0.4]);
        assert!((out[0] - 0.8).abs() < 1e-6, "{out:?}");
        assert!((out[1] - 0.2).abs() < 1e-6, "{out:?}");
        assert!((out[2] - 0.4).abs() < 1e-6, "{out:?}");
    }

    #[test]
    fn trilinear_hand_computed_corner_weight() {
        // All-black cube except node (1,1,1) = white. At the cube centre the
        // white corner's TRILINEAR weight is 0.5³ = 0.125 (the design's chosen
        // scheme; Swift's tetrahedral kernel would give 0.5 here).
        let mut lines = vec!["LUT_3D_SIZE 2".to_string()];
        for i in 0..8 {
            lines.push(if i == 7 { "1 1 1" } else { "0 0 0" }.to_string());
        }
        let lut = CubeLut::parse(&lines.join("\n")).unwrap();
        let out = lut.sample([0.5, 0.5, 0.5]);
        for c in out {
            assert!((c - 0.125).abs() < 1e-9, "trilinear centre weight: {out:?}");
        }
        assert_eq!(lut.sample([1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]);
        assert_eq!(lut.sample([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn sample_clamps_out_of_range_input() {
        let lut = CubeLut::parse(&cube_text(3)).unwrap();
        assert_eq!(lut.sample([-0.5, 1.5, 0.5]), lut.sample([0.0, 1.0, 0.5]));
    }

    // === load_cached ===

    #[test]
    fn load_cached_reads_and_caches() {
        let path = std::env::temp_dir().join(format!(
            "fronda-lut-cache-test-{}.cube",
            std::process::id()
        ));
        std::fs::write(&path, cube_text(2)).unwrap();
        let p = path.to_string_lossy().to_string();
        let a = load_cached(&p).expect("first load");
        let b = load_cached(&p).expect("cached load");
        assert!(Arc::ptr_eq(&a, &b), "second load hits the cache");
        assert_eq!(a.dimension, 2);
        // Upstream #343 loadUsesMemoryCacheAfterFirstRead: the memory cache
        // serves even after the file is gone (path-only key, no stat).
        std::fs::remove_file(&path).unwrap();
        let c = load_cached(&p).expect("cache survives file removal");
        assert!(Arc::ptr_eq(&a, &c));
    }

    #[test]
    fn load_cached_missing_file_errors() {
        assert!(load_cached("/nonexistent/fronda-no-such.cube").is_err());
    }
}
