//! The keystrokes the Twiddler sends for chords while data is being collected.
//!
//! Each chord in a config sends a different single keystroke, so the host can tell which chord was pressed.
//! Single keystrokes are used because a config can hold far more of them than multi-character outputs.

/// One key sent together with modifier keys.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Keystroke {
    /// USB HID modifier bitmap.
    pub modifiers: u8,
    /// USB HID keyboard usage id.
    pub key: u8,
}

// the usage ids of the letters a-z followed by the digits 1-9 and 0
const FIRST_KEY: u8 = 0x04;
const N_KEYS: usize = 36;
// in the Twiddler config format, this value in the modifier position marks a multi-character output
const MULTI_CHARACTER_MARKER: u8 = 0xFF;

/// The number of distinct keystrokes available for identifying chords.
pub const N_KEYSTROKES: usize = N_KEYS * MULTI_CHARACTER_MARKER as usize;

impl Keystroke {
    /// The `index`th keystroke used for identifying chords, if there are that many.
    pub fn from_index(index: usize) -> Option<Keystroke> {
        (index < N_KEYSTROKES).then(|| Keystroke {
            modifiers: (index / N_KEYS) as u8,
            key: FIRST_KEY + (index % N_KEYS) as u8,
        })
    }

    /// The inverse of `from_index`; `None` for keystrokes that are not used for identifying chords.
    pub fn index(&self) -> Option<usize> {
        let key_index = self.key.checked_sub(FIRST_KEY)? as usize;
        (key_index < N_KEYS && self.modifiers != MULTI_CHARACTER_MARKER).then(|| self.modifiers as usize * N_KEYS + key_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_inverts_from_index() {
        for index in 0..N_KEYSTROKES {
            let keystroke = Keystroke::from_index(index).unwrap();
            assert_ne!(keystroke.modifiers, MULTI_CHARACTER_MARKER);
            assert_eq!(keystroke.index(), Some(index));
        }
    }

    #[test]
    fn from_index_is_bounded() {
        assert_eq!(Keystroke::from_index(N_KEYSTROKES), None);
    }

    #[test]
    fn other_keystrokes_have_no_index() {
        let space = 0x2c;
        assert_eq!(Keystroke { modifiers: 0, key: space }.index(), None);
        assert_eq!(Keystroke { modifiers: 0, key: FIRST_KEY - 1 }.index(), None);
        assert_eq!(Keystroke { modifiers: MULTI_CHARACTER_MARKER, key: FIRST_KEY }.index(), None);
    }
}
