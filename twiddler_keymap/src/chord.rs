//! Chords on a Twiddler 3: which buttons are held down together.
//!
//! The Twiddler has four thumb buttons (num, alt, ctrl, shift) and four rows of three finger buttons (left, middle,
//! right), one row per finger. Only chords worth considering are representable: each finger presses a
//! contiguous run of the buttons in its row, and the thumb presses at most one button. (The firmware does not
//! look up chords with two thumb buttons: it sends a held modifier key instead of the chord's output.)

use std::fmt;

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::VariantArray;

pub const N_FINGERS: usize = 4;
const N_THUMB_BUTTONS: usize = 4;
const N_COLUMNS: usize = 3;

// in the Twiddler's 16-bit button mask, each group of four bits holds one thumb button followed by one finger's row
const MASK_BITS_PER_ROW: usize = 1 + N_COLUMNS;
const _: () = assert!(N_THUMB_BUTTONS == N_FINGERS && N_FINGERS * MASK_BITS_PER_ROW == u16::BITS as usize);

/// The buttons one finger presses in its row.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, VariantArray)]
pub enum FingerState {
    Lifted,
    L,
    M,
    R,
    LM,
    MR,
    LMR,
}

impl FingerState {
    /// Bit `i` is set if the button in column `i` (left to right) is pressed.
    const fn column_bits(self) -> u16 {
        match self {
            FingerState::Lifted => 0b000,
            FingerState::L => 0b001,
            FingerState::M => 0b010,
            FingerState::R => 0b100,
            FingerState::LM => 0b011,
            FingerState::MR => 0b110,
            FingerState::LMR => 0b111,
        }
    }
}

/// The buttons the thumb presses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, VariantArray)]
pub enum ThumbState {
    Lifted,
    Num,
    Alt,
    Ctrl,
    Shift,
}

impl ThumbState {
    /// Bit `i` is set if thumb button `i` (in the order num, alt, ctrl, shift) is pressed.
    const fn button_bits(self) -> u16 {
        match self {
            ThumbState::Lifted => 0b0000,
            ThumbState::Num => 0b0001,
            ThumbState::Alt => 0b0010,
            ThumbState::Ctrl => 0b0100,
            ThumbState::Shift => 0b1000,
        }
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
#[error("a chord must press at least one finger button")]
pub struct NoFingerButton;

/// A set of buttons held down together, including at least one finger button
/// (the Twiddler does not accept chords made only of thumb buttons).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(try_from = "ChordParts", into = "ChordParts")]
pub struct Chord {
    thumb: ThumbState,
    fingers: [FingerState; N_FINGERS],
}

impl Chord {
    /// `fingers` are ordered from the row nearest the thumb buttons to the row furthest from them.
    pub fn new(thumb: ThumbState, fingers: [FingerState; N_FINGERS]) -> Result<Self, NoFingerButton> {
        if fingers == [FingerState::Lifted; N_FINGERS] {
            Err(NoFingerButton)
        } else {
            Ok(Chord { thumb, fingers })
        }
    }

    pub fn thumb(&self) -> ThumbState {
        self.thumb
    }

    pub fn fingers(&self) -> [FingerState; N_FINGERS] {
        self.fingers
    }

    /// Every chord, each exactly once.
    pub fn all() -> impl Iterator<Item = Chord> {
        ThumbState::VARIANTS.iter().flat_map(|thumb| {
            std::iter::repeat_n(FingerState::VARIANTS.iter().copied(), N_FINGERS)
                .multi_cartesian_product()
                .filter_map(|fingers| {
                    let fingers = fingers.try_into().expect("the product has one entry per finger");
                    Chord::new(*thumb, fingers).ok()
                })
        })
    }

    /// The chord in the Twiddler config format: bit `4 * row` is thumb button `row`, and the three bits above it
    /// are the left, middle and right buttons of finger `row`.
    pub fn button_mask(&self) -> u16 {
        (0..N_FINGERS)
            .map(|row| {
                let thumb_bit = (self.thumb.button_bits() >> row) & 1;
                (thumb_bit | (self.fingers[row].column_bits() << 1)) << (row * MASK_BITS_PER_ROW)
            })
            .fold(0, |mask, row_bits| mask | row_bits)
    }

    pub fn n_buttons(&self) -> u32 {
        self.button_mask().count_ones()
    }

    /// Whether every button of `other` is also a button of this chord.
    pub fn contains(&self, other: &Chord) -> bool {
        self.button_mask() & other.button_mask() == other.button_mask()
    }
}

/// Draws the buttons as they are laid out on the device: the thumb buttons, then one row per finger.
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fn symbol(pressed: bool) -> char {
            if pressed { '⚫' } else { '⚪' }
        }
        let thumb_row: String = (0..N_THUMB_BUTTONS).map(|button| symbol((self.thumb.button_bits() >> button) & 1 == 1)).collect();
        writeln!(f, "{}", thumb_row)?;
        for finger in self.fingers {
            let finger_row: String = (0..N_COLUMNS).map(|column| symbol((finger.column_bits() >> column) & 1 == 1)).collect();
            // the thumb has one more button than the fingers
            writeln!(f, " {}", finger_row)?;
        }
        Ok(())
    }
}

/// The serialized form of a chord, which is not guaranteed to press a finger button.
#[derive(Serialize, Deserialize)]
#[serde(rename = "Chord")]
struct ChordParts {
    thumb: ThumbState,
    fingers: [FingerState; N_FINGERS],
}

impl TryFrom<ChordParts> for Chord {
    type Error = NoFingerButton;

    fn try_from(parts: ChordParts) -> Result<Self, Self::Error> {
        Chord::new(parts.thumb, parts.fingers)
    }
}

impl From<Chord> for ChordParts {
    fn from(chord: Chord) -> Self {
        ChordParts { thumb: chord.thumb, fingers: chord.fingers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use FingerState::{Lifted, L, LM, LMR, M, MR, R};

    #[test]
    fn chord_without_finger_buttons_is_rejected() {
        for thumb in ThumbState::VARIANTS {
            assert_eq!(Chord::new(*thumb, [Lifted; N_FINGERS]), Err(NoFingerButton));
        }
    }

    #[test]
    fn all_enumerates_every_chord_once() {
        let n_finger_shapes = FingerState::VARIANTS.len().pow(N_FINGERS as u32) - 1;
        let chords: Vec<Chord> = Chord::all().collect();
        assert_eq!(chords.len(), ThumbState::VARIANTS.len() * n_finger_shapes);
        assert_eq!(chords.iter().collect::<HashSet<_>>().len(), chords.len());
        assert_eq!(chords.iter().filter(|chord| chord.thumb() == ThumbState::Lifted).count(), 2400);
        assert_eq!(chords.len(), 12000);
    }

    #[test]
    fn button_masks_are_distinct() {
        let masks: HashSet<u16> = Chord::all().map(|chord| chord.button_mask()).collect();
        assert_eq!(masks.len(), Chord::all().count());
    }

    #[test]
    fn button_mask_matches_config_format() {
        // expected values are the masks twidlk writes for these chords
        let cases = [
            (ThumbState::Lifted, [L, Lifted, Lifted, Lifted], 0x0002),
            (ThumbState::Lifted, [LM, Lifted, Lifted, Lifted], 0x0006),
            (ThumbState::Lifted, [MR, Lifted, Lifted, Lifted], 0x000c),
            (ThumbState::Lifted, [LMR, Lifted, Lifted, Lifted], 0x000e),
            (ThumbState::Lifted, [Lifted, Lifted, Lifted, R], 0x8000),
            (ThumbState::Lifted, [M, M, M, M], 0x4444),
            (ThumbState::Num, [M, Lifted, Lifted, Lifted], 0x0005),
            (ThumbState::Alt, [M, Lifted, Lifted, Lifted], 0x0014),
            (ThumbState::Ctrl, [M, Lifted, Lifted, Lifted], 0x0104),
            (ThumbState::Shift, [M, Lifted, Lifted, Lifted], 0x1004),
        ];
        for (thumb, fingers, mask) in cases {
            assert_eq!(Chord::new(thumb, fingers).unwrap().button_mask(), mask, "{:?} {:?}", thumb, fingers);
        }
    }

    #[test]
    fn n_buttons_counts_thumb_and_finger_buttons() {
        assert_eq!(Chord::new(ThumbState::Num, [LMR, Lifted, M, Lifted]).unwrap().n_buttons(), 5);
    }

    #[test]
    fn contains_compares_buttons() {
        let small = Chord::new(ThumbState::Lifted, [L, Lifted, Lifted, Lifted]).unwrap();
        let large = Chord::new(ThumbState::Num, [LM, Lifted, R, Lifted]).unwrap();
        let other = Chord::new(ThumbState::Num, [M, Lifted, R, Lifted]).unwrap();
        assert!(large.contains(&small) && large.contains(&large) && large.contains(&other));
        assert!(!small.contains(&large) && !other.contains(&small));
    }

    #[test]
    fn display_draws_the_button_grid() {
        let chord = Chord::new(ThumbState::Ctrl, [L, Lifted, MR, LMR]).unwrap();
        assert_eq!(chord.to_string(), "⚪⚪⚫⚪\n ⚫⚪⚪\n ⚪⚪⚪\n ⚪⚫⚫\n ⚫⚫⚫\n");
    }

    #[test]
    fn serialization_round_trips() {
        for chord in Chord::all() {
            let json = serde_json::to_string(&chord).unwrap();
            assert_eq!(serde_json::from_str::<Chord>(&json).unwrap(), chord);
        }
    }

    #[test]
    fn deserializing_a_chord_without_finger_buttons_fails() {
        let json = r#"{"thumb":"Num","fingers":["Lifted","Lifted","Lifted","Lifted"]}"#;
        assert!(serde_json::from_str::<Chord>(json).is_err());
    }
}
