//! Ported from PalmierPro Tests/Rendering/TransformCropTests.swift
//!
//! Most transform/crop behavior is already covered by INS-001 through INS-011
//! in timeline_core's inspector tests. This file ports additional edge-case tests.

use core_model::{Crop, Transform};

/// Swift: topLeftIsCenterMinusHalfSize
#[test]
fn port_transform_top_left_derivation() {
    let t = Transform {
        center_x: 0.6,
        center_y: 0.4,
        width: 0.4,
        height: 0.2,
        ..Default::default()
    };
    let (x, y) = t.top_left();
    assert!((x - 0.4).abs() < 1e-9); // 0.6 - 0.2
    assert!((y - 0.3).abs() < 1e-9); // 0.4 - 0.1
}

/// Swift: identityCropHasAllZeroInsets
#[test]
fn port_crop_identity() {
    let crop = Crop::default();
    assert!(crop.is_identity());
    assert!(!Crop {
        left: 0.1,
        ..Default::default()
    }
    .is_identity());
}

/// Swift: visibleFractionsSubtractInsets
#[test]
fn port_crop_visible_fractions() {
    let crop = Crop {
        left: 0.1,
        top: 0.2,
        right: 0.3,
        bottom: 0.4,
    };
    assert!((crop.visible_width_fraction() - 0.6).abs() < 1e-9); // 1 - 0.1 - 0.3
    assert!((crop.visible_height_fraction() - 0.4).abs() < 1e-9); // 1 - 0.2 - 0.4
}
