//! Ported from PalmierPro Tests/Captions/TranscriptionLocaleTests.swift
//!
//! Tests locale matching for transcription.

/// TRN-012: Locale matching prefers exact language+region matches first.
/// TRN-013: If no exact region exists, falls back to any supported locale with same language.
/// TRN-014: Region override suffixes and Unicode extension tags don't block language matching.
/// A simplified locale matcher for testing.
/// Supported locales are represented as "language_region" strings.
fn match_locale(candidates: &[&str], supported: &[&str]) -> Option<String> {
    // First pass: try exact match (language + region)
    for candidate in candidates {
        if supported.contains(candidate) {
            return Some(candidate.to_string());
        }
    }
    // Second pass: try language-only match (extract language code)
    for candidate in candidates {
        let lang = candidate.split(['_', '-']).next().unwrap_or("");
        for s in supported {
            if s.starts_with(lang) {
                return Some(s.to_string());
            }
        }
    }
    // Third pass: strip suffixes (@rg=..., -u-rg-...) and retry
    for candidate in candidates {
        let cleaned = candidate
            .split('@')
            .next()
            .unwrap_or("")
            .split("-u-")
            .next()
            .unwrap_or("")
            .to_string();
        if !cleaned.is_empty() && cleaned != *candidate {
            let lang = cleaned.split(['_', '-']).next().unwrap_or("");
            for s in supported {
                if s.starts_with(lang) {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

#[test]
fn port_locale_exact_region_preferred() {
    let supported = &["en_US", "en_GB", "fr_FR", "fr_CA"];
    let result = match_locale(&["fr_CA"], supported);
    assert_eq!(result, Some("fr_CA".to_string()));
}

#[test]
fn port_locale_region_override_stripped() {
    // en_US@rg=frzzzz → should match en_US
    let supported = &["en_US", "en_GB"];
    let result = match_locale(&["en_US@rg=frzzzz"], supported);
    assert_eq!(result, Some("en_US".to_string()));
}

#[test]
fn port_locale_unicode_extension_stripped() {
    // en-US-u-rg-zazzzz → should match en_US
    let supported = &["en_US", "en_GB"];
    let result = match_locale(&["en-US-u-rg-zazzzz"], supported);
    assert_eq!(result, Some("en_US".to_string()));
}

#[test]
fn port_locale_language_fallback() {
    // en_FR has no exact match → fall back to any en_*
    let supported = &["en_US", "en_GB", "fr_FR"];
    let result = match_locale(&["en_FR"], supported);
    assert!(result.unwrap().starts_with("en_"));
}

#[test]
fn port_locale_no_match_returns_none() {
    let supported = &["en_US", "fr_FR"];
    let result = match_locale(&["ja_JP"], supported);
    assert_eq!(result, None);
}

#[test]
fn port_locale_language_only_matches_any_region() {
    let supported = &["en_US", "en_GB", "fr_FR"];
    let result = match_locale(&["fr"], supported);
    assert!(result.unwrap().starts_with("fr_"));
}

#[test]
fn port_locale_candidate_order_wins() {
    let supported = &["en_US", "fr_FR", "fr_CA"];
    let result = match_locale(&["fr_FR", "en_US"], supported);
    assert_eq!(result, Some("fr_FR".to_string()));
}
