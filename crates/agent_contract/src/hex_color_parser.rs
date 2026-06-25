//! Hex color string parsing and validation (see MUT-023 / upstream #72).
//!
//! Accepts `#RGB`, `#RRGGBB`, `#RRGGBBAA` formats.
//! Trims surrounding whitespace; rejects internal whitespace and non-hex characters.

/// Parse and validate a hex color string.
///
/// Returns the trimmed string on success, or an error message on failure.
pub fn parse_hex_color(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty hex color".to_string());
    }

    let without_prefix = trimmed
        .strip_prefix('#')
        .ok_or_else(|| "hex color must start with '#'".to_string())?;

    if without_prefix.contains(|c: char| c.is_whitespace()) {
        return Err("hex color contains internal whitespace".to_string());
    }

    match without_prefix.len() {
        3 | 6 | 8 => {
            if without_prefix.chars().all(|c| c.is_ascii_hexdigit()) {
                Ok(trimmed.to_string())
            } else {
                Err("hex color contains invalid characters".to_string())
            }
        }
        _ => Err("hex color must be #RGB, #RRGGBB, or #RRGGBBAA".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_short_form() {
        let result = parse_hex_color("#fff");
        assert_eq!(result.unwrap(), "#fff");
    }

    #[test]
    fn valid_long_form() {
        let result = parse_hex_color("#ffffff");
        assert_eq!(result.unwrap(), "#ffffff");
    }

    #[test]
    fn valid_alpha_form() {
        let result = parse_hex_color("#ffffffff");
        assert_eq!(result.unwrap(), "#ffffffff");
    }

    #[test]
    fn trims_whitespace() {
        let result = parse_hex_color("  #FF0000\n");
        assert_eq!(result.unwrap(), "#FF0000");
    }

    #[test]
    fn rejects_empty_string() {
        assert!(parse_hex_color("").is_err());
    }

    #[test]
    fn rejects_missing_hash() {
        assert!(parse_hex_color("ff0000").is_err());
    }

    #[test]
    fn rejects_internal_whitespace() {
        assert!(parse_hex_color("#FF 0000").is_err());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_hex_color("#ffff").is_err());
    }

    #[test]
    fn rejects_non_hex_chars() {
        assert!(parse_hex_color("#GGGGGG").is_err());
    }
}
