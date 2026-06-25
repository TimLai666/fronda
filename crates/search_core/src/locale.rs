//! BCP-47 locale matching with fallback (TRN-012 to TRN-015).
//!
//! TRN-012: Exact language+region matches preferred.
//! TRN-013: Fall back to same-language if no region match.
//! TRN-014: Unicode extension tags (`@rg=...`, `-u-rg-...`) are stripped.
//! TRN-015: Return `None` if no supported language matches.

/// Strips Unicode locale extension tags from a BCP-47 tag (TRN-014).
///
/// Handles:
/// - `-u-rg-...` extensions (e.g. `en-US-u-rg-gbzzzz` → `en-US`)
/// - `@rg=...` suffixes (e.g. `en-US@rg=GB` → `en-US`)
/// - Other private-use extensions preceded by `-x-`
pub fn strip_unicode_extensions(tag: &str) -> String {
    // Strip `-u-` extensions (e.g. `en-US-u-rg-gbzzzz`)
    if let Some(pos) = tag.find("-u-") {
        let base = &tag[..pos];
        return base.to_string();
    }
    // Strip `@rg=...` suffixes (e.g. `en-US@rg=GB`)
    if let Some(pos) = tag.find('@') {
        let base = &tag[..pos];
        return base.to_string();
    }
    // Strip `-x-` private use
    if let Some(pos) = tag.find("-x-") {
        let base = &tag[..pos];
        return base.to_string();
    }
    tag.to_string()
}

/// Parse a BCP-47 tag into (language, region) where region may be empty.
fn parse_locale(tag: &str) -> (String, String) {
    let cleaned = strip_unicode_extensions(tag);
    let parts: Vec<&str> = cleaned.splitn(3, |c: char| c == '-' || c == '_').collect();
    let lang = parts.first().unwrap_or(&"und").to_string().to_lowercase();
    let region = parts.get(1).map(|r| r.to_uppercase()).unwrap_or_default();
    (lang, region)
}

/// Match a requested locale against a list of supported locales (TRN-012..015).
///
/// Returns the best-matching supported locale string, or `None` if no match.
///
/// Matching order:
/// 1. Exact language+region match (TRN-012)
/// 2. Same language, any region (TRN-013)
/// 3. No match → None (TRN-015)
///
/// Unicode extension tags are stripped before matching (TRN-014).
pub fn match_locale(requested: &str, supported: &[String]) -> Option<String> {
    let (req_lang, req_region) = parse_locale(requested);

    if req_lang.is_empty() || req_lang == "und" {
        return None;
    }

    // First pass: look for exact language+region match (TRN-012)
    if !req_region.is_empty() {
        for supported_tag in supported {
            let (sup_lang, sup_region) = parse_locale(supported_tag);
            if sup_lang == req_lang && sup_region == req_region {
                return Some(supported_tag.clone());
            }
        }
    }

    // Second pass: same language, any region (TRN-013)
    for supported_tag in supported {
        let (sup_lang, _) = parse_locale(supported_tag);
        if sup_lang == req_lang {
            return Some(supported_tag.clone());
        }
    }

    // No match (TRN-015)
    None
}

/// Returns true if a requested locale can be matched against a list of
/// supported locales. Convenience wrapper around `match_locale`.
pub fn locale_is_supported(requested: &str, supported: &[String]) -> bool {
    match_locale(requested, supported).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // TRN-014: Unicode extension stripping
    // -----------------------------------------------------------------------

    #[test]
    fn trn_014_strips_u_extension() {
        assert_eq!(
            strip_unicode_extensions("en-US-u-rg-gbzzzz"),
            "en-US",
            "TRN-014: strips -u-rg- extension"
        );
    }

    #[test]
    fn trn_014_strips_at_rg_suffix() {
        assert_eq!(
            strip_unicode_extensions("zh-TW@rg=GB"),
            "zh-TW",
            "TRN-014: strips @rg= suffix"
        );
    }

    #[test]
    fn trn_014_preserves_clean_tag() {
        assert_eq!(
            strip_unicode_extensions("en-US"),
            "en-US",
            "TRN-014: clean tag unchanged"
        );
    }

    #[test]
    fn trn_014_strips_x_private_use() {
        assert_eq!(
            strip_unicode_extensions("en-x-myprivate"),
            "en",
            "TRN-014: strips -x- private use"
        );
    }

    #[test]
    fn trn_014_handles_empty() {
        assert_eq!(strip_unicode_extensions(""), "", "TRN-014: empty input");
    }

    // -----------------------------------------------------------------------
    // TRN-012: Exact language+region match
    // -----------------------------------------------------------------------

    #[test]
    fn trn_012_exact_match() {
        let supported = vec![
            "en-US".to_string(),
            "en-GB".to_string(),
            "zh-TW".to_string(),
        ];
        let result = match_locale("en-US", &supported);
        assert_eq!(result, Some("en-US".to_string()));
    }

    #[test]
    fn trn_012_exact_match_case_insensitive_language() {
        let supported = vec!["EN-us".to_string(), "en-GB".to_string()];
        let result = match_locale("en-US", &supported);
        assert_eq!(
            result,
            Some("EN-us".to_string()),
            "TRN-012: case-insensitive match"
        );
    }

    // -----------------------------------------------------------------------
    // TRN-013: Same-language fallback
    // -----------------------------------------------------------------------

    #[test]
    fn trn_013_fallback_same_language() {
        let supported = vec!["en-GB".to_string(), "fr-FR".to_string()];
        // Request en-US, but only en-GB is supported → fallback to en-GB
        let result = match_locale("en-US", &supported);
        assert_eq!(
            result,
            Some("en-GB".to_string()),
            "TRN-013: fallback to same language"
        );
    }

    #[test]
    fn trn_013_fallback_when_no_region() {
        let supported = vec!["en".to_string()];
        let result = match_locale("en-US", &supported);
        assert_eq!(
            result,
            Some("en".to_string()),
            "TRN-013: fallback to language-only"
        );
    }

    // -----------------------------------------------------------------------
    // TRN-014: Matching with extension tags
    // -----------------------------------------------------------------------

    #[test]
    fn trn_014_matches_with_extension() {
        let supported = vec!["en-US".to_string(), "en-GB".to_string()];
        // Request contains -u-rg- extension, should match en-US after strip
        let result = match_locale("en-US-u-rg-gbzzzz", &supported);
        assert_eq!(
            result,
            Some("en-US".to_string()),
            "TRN-014: extension stripped before match"
        );
    }

    #[test]
    fn trn_014_matches_with_at_rg() {
        let supported = vec!["zh-TW".to_string(), "zh-CN".to_string()];
        let result = match_locale("zh-TW@rg=GB", &supported);
        assert_eq!(
            result,
            Some("zh-TW".to_string()),
            "TRN-014: @rg stripped before match"
        );
    }

    // -----------------------------------------------------------------------
    // TRN-015: No match
    // -----------------------------------------------------------------------

    #[test]
    fn trn_015_no_match_at_all() {
        let supported = vec!["fr-FR".to_string(), "de-DE".to_string()];
        let result = match_locale("en-US", &supported);
        assert_eq!(result, None, "TRN-015: no match returns None");
    }

    #[test]
    fn trn_015_empty_supported_list() {
        let supported: Vec<String> = vec![];
        let result = match_locale("en-US", &supported);
        assert_eq!(result, None, "TRN-015: empty supported list");
    }

    #[test]
    fn trn_015_undetermined_language() {
        let supported = vec!["en-US".to_string()];
        let result = match_locale("und", &supported);
        assert_eq!(result, None, "TRN-015: undetermined language");
    }

    #[test]
    fn trn_015_empty_request() {
        let supported = vec!["en-US".to_string()];
        let result = match_locale("", &supported);
        assert_eq!(result, None, "TRN-015: empty request");
    }

    // -----------------------------------------------------------------------
    // locale_is_supported convenience
    // -----------------------------------------------------------------------

    #[test]
    fn locale_is_supported_true() {
        let supported = vec!["en-US".to_string(), "zh-TW".to_string()];
        assert!(locale_is_supported("en-US", &supported));
        assert!(locale_is_supported("zh-TW", &supported));
    }

    #[test]
    fn locale_is_supported_false() {
        let supported = vec!["fr-FR".to_string()];
        assert!(!locale_is_supported("en-US", &supported));
    }
}
