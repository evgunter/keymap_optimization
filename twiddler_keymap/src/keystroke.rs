//! The keystrokes the Twiddler sends for chords while data is being collected.
//!
//! Each chord in a config sends a different single keystroke, so the host can tell which chord was pressed.
//! Single keystrokes are used because a config can hold far more of them than multi-character outputs.
//!
//! The Twiddler also sends keystrokes of its own accord: for a chord that has no output but includes the ctrl,
//! shift or alt thumb button, it sends the output of the chord's finger buttons together with that button's
//! modifier key. The keystrokes used here leave those three modifiers alone, so such a keystroke can be told
//! apart and still identifies the chord that was pressed.

use crate::chord::ThumbState;

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

// the modifiers the Twiddler sends for its thumb buttons: left ctrl, left shift and left alt
const CTRL_MODIFIER: u8 = 0x01;
const SHIFT_MODIFIER: u8 = 0x02;
const ALT_MODIFIER: u8 = 0x04;
const THUMB_MODIFIERS: u8 = CTRL_MODIFIER | SHIFT_MODIFIER | ALT_MODIFIER;
// the other modifiers (left gui, right ctrl, right shift, right alt, right gui) tell chords' keystrokes apart
const N_FREE_MODIFIERS: u32 = THUMB_MODIFIERS.count_zeros();
const FREE_MODIFIERS_SHIFT: u32 = THUMB_MODIFIERS.count_ones();
const _: () = assert!(THUMB_MODIFIERS == (1 << FREE_MODIFIERS_SHIFT) - 1);

/// The number of distinct keystrokes available for identifying chords.
pub const N_KEYSTROKES: usize = N_KEYS << N_FREE_MODIFIERS;

impl Keystroke {
    /// The `index`th keystroke used for identifying chords, if there are that many.
    pub fn from_index(index: usize) -> Option<Keystroke> {
        (index < N_KEYSTROKES).then(|| Keystroke {
            modifiers: ((index / N_KEYS) as u8) << FREE_MODIFIERS_SHIFT,
            key: FIRST_KEY + (index % N_KEYS) as u8,
        })
    }

    /// The inverse of `from_index`; `None` for keystrokes that are not used for identifying chords.
    pub fn index(&self) -> Option<usize> {
        let key_index = self.key.checked_sub(FIRST_KEY)? as usize;
        (key_index < N_KEYS && self.modifiers & THUMB_MODIFIERS == 0)
            .then(|| (self.modifiers >> FREE_MODIFIERS_SHIFT) as usize * N_KEYS + key_index)
    }

    /// Splits off the modifier the Twiddler adds for a thumb button: returns that button (`Lifted` if there is
    /// no such modifier) and the keystroke without it. `None` if the modifiers of several thumb buttons are held.
    pub fn split_thumb(&self) -> Option<(ThumbState, Keystroke)> {
        let thumb = match self.modifiers & THUMB_MODIFIERS {
            0 => ThumbState::Lifted,
            CTRL_MODIFIER => ThumbState::Ctrl,
            SHIFT_MODIFIER => ThumbState::Shift,
            ALT_MODIFIER => ThumbState::Alt,
            _ => return None,
        };
        Some((thumb, Keystroke { modifiers: self.modifiers & !THUMB_MODIFIERS, key: self.key }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_inverts_from_index() {
        assert_eq!(N_KEYSTROKES, 1152);
        for index in 0..N_KEYSTROKES {
            let keystroke = Keystroke::from_index(index).unwrap();
            assert_eq!(keystroke.modifiers & THUMB_MODIFIERS, 0);
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
        assert_eq!(Keystroke { modifiers: CTRL_MODIFIER, key: FIRST_KEY }.index(), None);
    }

    #[test]
    fn thumb_modifiers_are_split_off() {
        let keystroke = Keystroke::from_index(500).unwrap();
        assert_eq!(keystroke.split_thumb(), Some((ThumbState::Lifted, keystroke)));
        for (modifier, thumb) in [(CTRL_MODIFIER, ThumbState::Ctrl), (SHIFT_MODIFIER, ThumbState::Shift), (ALT_MODIFIER, ThumbState::Alt)] {
            let with_thumb = Keystroke { modifiers: keystroke.modifiers | modifier, key: keystroke.key };
            assert_eq!(with_thumb.split_thumb(), Some((thumb, keystroke)));
        }
        let two_thumbs = Keystroke { modifiers: keystroke.modifiers | CTRL_MODIFIER | ALT_MODIFIER, key: keystroke.key };
        assert_eq!(two_thumbs.split_thumb(), None);
    }
}
