use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub key: char,
}

impl FromStr for KeyCombo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            format!(
                "invalid hotkey '{s}' — use alt/ctrl/shift plus a letter or digit, e.g. 'alt+a'"
            )
        };
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let key_part = parts.pop().ok_or_else(invalid)?;
        let mut key_chars = key_part.chars();
        let key = match (key_chars.next(), key_chars.next()) {
            (Some(c), None) if c.is_ascii_alphanumeric() => c.to_ascii_uppercase(),
            _ => return Err(invalid()),
        };
        let mut combo = KeyCombo {
            alt: false,
            ctrl: false,
            shift: false,
            key,
        };
        for part in parts {
            let flag = match part.to_ascii_lowercase().as_str() {
                "alt" => &mut combo.alt,
                "ctrl" => &mut combo.ctrl,
                "shift" => &mut combo.shift,
                _ => return Err(invalid()),
            };
            if *flag {
                return Err(invalid());
            }
            *flag = true;
        }
        if !(combo.alt || combo.ctrl || combo.shift) {
            return Err(invalid());
        }
        Ok(combo)
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
        ] {
            if on {
                f.write_str(name)?;
            }
        }
        write!(f, "{}", self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<KeyCombo, String> {
        s.parse()
    }

    #[test]
    fn parses_alt_letter() {
        assert_eq!(
            parse("alt+a").unwrap(),
            KeyCombo {
                alt: true,
                ctrl: false,
                shift: false,
                key: 'A'
            }
        );
    }

    #[test]
    fn parses_case_insensitively() {
        assert_eq!(parse("ALT+A").unwrap(), parse("alt+a").unwrap());
        assert_eq!(
            parse("Shift+Ctrl+x").unwrap(),
            parse("ctrl+shift+X").unwrap()
        );
    }

    #[test]
    fn parses_combined_modifiers_and_digit() {
        let combo = parse("ctrl+shift+1").unwrap();
        assert!(combo.ctrl && combo.shift && !combo.alt);
        assert_eq!(combo.key, '1');
    }

    #[test]
    fn tolerates_spaces_around_parts() {
        assert_eq!(parse(" alt + n ").unwrap(), parse("alt+n").unwrap());
    }

    #[test]
    fn rejects_invalid_values() {
        for value in [
            "a",
            "",
            "alt",
            "alt+",
            "alt+ab",
            "alt+f1",
            "alt+/",
            "meta+a",
            "alt+alt+a",
            "+a",
        ] {
            assert!(parse(value).is_err(), "expected '{value}' to be rejected");
        }
    }

    #[test]
    fn displays_in_canonical_order() {
        assert_eq!(
            parse("shift+alt+ctrl+k").unwrap().to_string(),
            "Ctrl+Alt+Shift+K"
        );
        assert_eq!(parse("alt+a").unwrap().to_string(), "Alt+A");
    }
}
