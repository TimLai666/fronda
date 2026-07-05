//! Minimal keystroke-to-text editing shared by inline text fields
//! (chat composer, timeline tab rename). IME composition is not yet
//! supported — printable input comes from `Keystroke::key_char`.

/// Apply one editing keystroke to `text`. Returns true if the text changed.
/// Enter/Escape are the caller's to handle; this covers backspace, space
/// (whose `key_char` is None on Windows), and printable characters,
/// ignoring control/platform/function chords.
pub fn apply_editing_keystroke(text: &mut String, keystroke: &gpui::Keystroke) -> bool {
    match keystroke.key.as_str() {
        "backspace" => text.pop().is_some(),
        "space" => {
            text.push(' ');
            true
        }
        _ => {
            let mods = &keystroke.modifiers;
            if mods.control || mods.platform || mods.function {
                return false;
            }
            match keystroke.key_char.as_deref() {
                Some(ch) if !ch.chars().any(char::is_control) => {
                    text.push_str(ch);
                    true
                }
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Keystroke, Modifiers};

    fn ks(key: &str, key_char: Option<&str>, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            key: key.into(),
            key_char: key_char.map(Into::into),
            modifiers,
        }
    }

    #[test]
    fn printable_space_backspace_edit() {
        let mut t = String::new();
        assert!(apply_editing_keystroke(&mut t, &ks("a", Some("a"), Modifiers::default())));
        assert!(apply_editing_keystroke(&mut t, &ks("space", None, Modifiers::default())));
        assert!(apply_editing_keystroke(&mut t, &ks("b", Some("B"), Modifiers::default())));
        assert_eq!(t, "a B");
        assert!(apply_editing_keystroke(&mut t, &ks("backspace", None, Modifiers::default())));
        assert_eq!(t, "a ");
        t.clear();
        assert!(
            !apply_editing_keystroke(&mut t, &ks("backspace", None, Modifiers::default())),
            "empty backspace edits nothing"
        );
    }

    #[test]
    fn chords_and_non_printables_ignored() {
        let mut t = String::new();
        let cmd = Modifiers {
            platform: true,
            ..Default::default()
        };
        assert!(!apply_editing_keystroke(&mut t, &ks("s", Some("s"), cmd)));
        assert!(!apply_editing_keystroke(&mut t, &ks("left", None, Modifiers::default())));
        assert!(!apply_editing_keystroke(
            &mut t,
            &ks("tab", Some("\t"), Modifiers::default())
        ));
        assert!(t.is_empty());
    }
}
