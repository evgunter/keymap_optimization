//! A deck is the set of chords loaded onto the Twiddler for one data collection session.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use twidlk_rust::twiddler_config::{generate_bin_config, sort_chords, ChordOutput, ChordWithOutput, TwiddlerConfig};

use crate::chord::Chord;
use crate::keystroke::{Keystroke, N_KEYSTROKES};

/// The most chords a Twiddler 3 config can hold.
pub const MAX_CHORDS: usize = 1020;
const _: () = assert!(MAX_CHORDS <= N_KEYSTROKES);

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum DeckError {
    #[error("a deck can hold at most {MAX_CHORDS} chords, but {0} were given")]
    TooManyChords(usize),
    #[error("a deck cannot contain the same chord twice")]
    DuplicateChord,
    #[error("a deck cannot contain {0:?}, because the Twiddler sees it as a different chord")]
    NotRegistrable(Chord),
}

/// Distinct chords that the Twiddler can see, few enough to fit in one config.
/// The chord at position `i` sends keystroke `i`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(try_from = "Vec<Chord>", into = "Vec<Chord>")]
pub struct Deck {
    chords: Vec<Chord>,
}

impl Deck {
    pub fn new(chords: Vec<Chord>) -> Result<Self, DeckError> {
        if chords.len() > MAX_CHORDS {
            Err(DeckError::TooManyChords(chords.len()))
        } else if chords.iter().collect::<HashSet<_>>().len() != chords.len() {
            Err(DeckError::DuplicateChord)
        } else if let Some(chord) = chords.iter().find(|chord| !chord.registers_as_itself()) {
            Err(DeckError::NotRegistrable(*chord))
        } else {
            Ok(Deck { chords })
        }
    }

    pub fn chords(&self) -> &[Chord] {
        &self.chords
    }

    fn keystrokes(&self) -> impl Iterator<Item = (Chord, Keystroke)> {
        self.chords.iter().enumerate().map(|(index, chord)| {
            (*chord, Keystroke::from_index(index).expect("a deck has no more chords than there are keystrokes"))
        })
    }

    /// The chord that sends `keystroke`, if any chord in the deck does.
    pub fn chord_for(&self, keystroke: Keystroke) -> Option<Chord> {
        self.chords.get(keystroke.index()?).copied()
    }

    /// The contents of a `twiddler.cfg` file that makes each chord in the deck send its keystroke.
    pub fn config(&self) -> Vec<u8> {
        let mut chords: Vec<ChordWithOutput> = self.keystrokes().map(|(chord, keystroke)| ChordWithOutput {
            chord: twidlk_rust::Chord { keys: (0..u16::BITS as u16).filter(|bit| chord.button_mask() & (1 << bit) != 0).collect() },
            output: ChordOutput::SingleChord { modifier: keystroke.modifiers, key_code: keystroke.key },
        }).collect();
        // the Twiddler rejects configs whose chords are not in this order
        sort_chords(&mut chords);

        let config = TwiddlerConfig {
            version: (),
            // a chord's keystroke must be sent exactly once
            key_repeat: false,
            direct_key: false,
            joystick_left_click: false,
            disable_bluetooth: false,
            sticky_num: false,
            sticky_shift: false,
            haptic_feedback: false,

            sleep_timeout: 300,
            mouse_left_click_action: 0,
            mouse_middle_click_action: 0,
            mouse_right_click_action: 0,
            mouse_accel_factor: 255,
            key_repeat_delay: 100,

            chords,
        };
        generate_bin_config(&config).expect("writing a config to memory cannot fail")
    }
}

impl TryFrom<Vec<Chord>> for Deck {
    type Error = DeckError;

    fn try_from(chords: Vec<Chord>) -> Result<Self, Self::Error> {
        Deck::new(chords)
    }
}

impl From<Deck> for Vec<Chord> {
    fn from(deck: Deck) -> Self {
        deck.chords
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use twidlk_rust::twiddler_config::read_config;

    fn chords(n: usize) -> Vec<Chord> {
        let chords: Vec<Chord> = Chord::all().filter(Chord::registers_as_itself).take(n).collect();
        assert_eq!(chords.len(), n);
        chords
    }

    #[test]
    fn too_many_chords_are_rejected() {
        assert!(Deck::new(chords(MAX_CHORDS)).is_ok());
        assert_eq!(Deck::new(chords(MAX_CHORDS + 1)), Err(DeckError::TooManyChords(MAX_CHORDS + 1)));
    }

    #[test]
    fn duplicate_chords_are_rejected() {
        let mut with_duplicate = chords(3);
        with_duplicate.push(with_duplicate[0]);
        assert_eq!(Deck::new(with_duplicate), Err(DeckError::DuplicateChord));
    }

    #[test]
    fn chords_that_register_as_others_are_rejected() {
        let ghosting = Chord::all().find(|chord| !chord.registers_as_itself()).unwrap();
        assert_eq!(Deck::new(vec![ghosting]), Err(DeckError::NotRegistrable(ghosting)));
    }

    #[test]
    fn deserializing_an_invalid_deck_fails() {
        let mut with_duplicate = chords(3);
        with_duplicate.push(with_duplicate[0]);
        let json = serde_json::to_string(&with_duplicate).unwrap();
        assert!(serde_json::from_str::<Deck>(&json).is_err());
    }

    #[test]
    fn serialization_round_trips() {
        let deck = Deck::new(chords(MAX_CHORDS)).unwrap();
        let json = serde_json::to_string(&deck).unwrap();
        assert_eq!(serde_json::from_str::<Deck>(&json).unwrap(), deck);
    }

    #[test]
    fn each_chord_is_found_from_its_keystroke() {
        let deck = Deck::new(chords(MAX_CHORDS)).unwrap();
        for (chord, keystroke) in deck.keystrokes() {
            assert_eq!(deck.chord_for(keystroke), Some(chord));
        }
        assert_eq!(deck.chord_for(Keystroke::from_index(MAX_CHORDS).unwrap()), None);
    }

    #[test]
    fn config_maps_each_chord_to_its_keystroke() {
        let deck = Deck::new(chords(MAX_CHORDS)).unwrap();
        // TODO: twidlk's read_config fails on configs without multi-character outputs, because it always reads
        // one string location. we append one so that it can read back the rest.
        let mut config_bytes = deck.config();
        config_bytes.extend([0; 4]);
        let config = read_config(&config_bytes).unwrap();

        assert!(!config.key_repeat && !config.direct_key);
        let written: HashSet<(u16, Keystroke)> = config.chords.iter().map(|chord_with_output| {
            let mask = chord_with_output.chord.keys.iter().fold(0, |mask, bit| mask | (1 << bit));
            match chord_with_output.output {
                ChordOutput::SingleChord { modifier, key_code } => (mask, Keystroke { modifiers: modifier, key: key_code }),
                ref other => panic!("expected a single keystroke, got {:?}", other),
            }
        }).collect();
        let expected: HashSet<(u16, Keystroke)> = deck.keystrokes().map(|(chord, keystroke)| (chord.button_mask(), keystroke)).collect();
        assert_eq!(written, expected);

        let masks: Vec<u16> = config.chords.iter().map(|c| c.chord.keys.iter().fold(0, |mask, bit| mask | (1 << bit))).collect();
        assert!(masks.is_sorted(), "the Twiddler requires chords in order of their button masks");
    }
}
