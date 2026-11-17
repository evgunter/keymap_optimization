//! A deck is the set of chords loaded onto the Twiddler for one data collection session.

use std::collections::HashSet;
use std::io;

use itertools::Itertools;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use twidlk_rust::twiddler_config::{generate_bin_config, sort_chords, ChordOutput, ChordWithOutput, TwiddlerConfig};

use crate::chord::{Chord, ThumbState};
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
///
/// A deck also identifies chords it does not hold: for a chord with the ctrl, shift or alt thumb button that
/// has no output of its own, the Twiddler sends the keystroke of the chord's finger buttons with a modifier
/// added (see `Keystroke::split_thumb`).
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

    /// Every chord the Twiddler can see that uses no thumb button or the num button, fewest buttons first.
    /// Loaded onto the Twiddler, it identifies every chord the Twiddler can see.
    pub fn full() -> Self {
        let chords = Chord::all()
            .filter(|chord| chord.registers_as_itself() && matches!(chord.thumb(), ThumbState::Lifted | ThumbState::Num))
            .sorted_by_key(|chord| (chord.n_buttons(), chord.button_mask()))
            .collect();
        Deck::new(chords).expect("the chords are distinct, registrable, and few enough for one config")
    }

    /// The chord that was pressed, given the keystroke the Twiddler sent for it while this deck was loaded.
    /// `None` if the keystroke cannot have come from this deck.
    pub fn chord_for(&self, keystroke: Keystroke) -> Option<Chord> {
        let (thumb, keystroke) = keystroke.split_thumb()?;
        let chord = *self.chords.get(keystroke.index()?)?;
        match thumb {
            ThumbState::Lifted => Some(chord),
            // the Twiddler only adds a thumb button's modifier to the keystroke of the remaining buttons if the
            // whole chord has no keystroke of its own, and it does not do so for two thumb buttons
            thumb => (chord.thumb() == ThumbState::Lifted && !self.chords.contains(&chord.with_thumb(thumb))).then(|| chord.with_thumb(thumb)),
        }
    }

    /// Whether pressing `chord` with this deck loaded makes the Twiddler send a keystroke that identifies it.
    pub fn identifies(&self, chord: &Chord) -> bool {
        self.chords.contains(chord)
            || (matches!(chord.thumb(), ThumbState::Ctrl | ThumbState::Shift | ThumbState::Alt) && self.chords.contains(&chord.with_thumb(ThumbState::Lifted)))
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

    /// Writes `<name>.cfg`, the config to load onto the Twiddler, and `<name>.json`, the deck itself, which the
    /// programs that read the Twiddler need in order to interpret that config's keystrokes.
    pub fn write_files(&self, directory: &Path, name: &str) -> io::Result<[PathBuf; 2]> {
        let paths = ["cfg", "json"].map(|extension| directory.join(format!("{}.{}", name, extension)));
        std::fs::write(&paths[0], self.config())?;
        std::fs::write(&paths[1], serde_json::to_string(self)?)?;
        Ok(paths)
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

    /// The keystroke the Twiddler sends for a chord with a thumb button that has no keystroke of its own.
    fn fallback_keystroke(deck: &Deck, chord: &Chord) -> Keystroke {
        let (_, keystroke) = deck.keystrokes().find(|(held, _)| *held == chord.with_thumb(ThumbState::Lifted)).unwrap();
        let modifier = match chord.thumb() {
            ThumbState::Ctrl => 0x01,
            ThumbState::Shift => 0x02,
            ThumbState::Alt => 0x04,
            other => panic!("the Twiddler adds no modifier for {:?}", other),
        };
        Keystroke { modifiers: keystroke.modifiers | modifier, key: keystroke.key }
    }

    #[test]
    fn the_full_deck_identifies_every_chord_the_twiddler_can_see() {
        let deck = Deck::full();
        assert_eq!(deck.chords().len(), 800);
        let registrable: Vec<Chord> = Chord::all().filter(Chord::registers_as_itself).collect();
        assert_eq!(registrable.len(), 2000);

        let mut keystrokes = HashSet::new();
        for chord in &registrable {
            assert!(deck.identifies(chord), "{:?}", chord);
            let keystroke = match deck.keystrokes().find(|(held, _)| held == chord) {
                Some((_, keystroke)) => keystroke,
                None => fallback_keystroke(&deck, chord),
            };
            assert_eq!(deck.chord_for(keystroke), Some(*chord));
            assert!(keystrokes.insert(keystroke), "two chords send {:?}", keystroke);
        }
    }

    #[test]
    fn thumb_modifiers_identify_only_chords_without_a_keystroke_of_their_own() {
        let [finger_chord, other] = chords(2).try_into().unwrap();
        assert_eq!(finger_chord.thumb(), ThumbState::Lifted);
        let ctrl_chord = finger_chord.with_thumb(ThumbState::Ctrl);
        let num_chord = finger_chord.with_thumb(ThumbState::Num);

        let deck = Deck::new(vec![finger_chord, other, num_chord]).unwrap();
        assert!(deck.identifies(&ctrl_chord) && deck.identifies(&num_chord));
        assert_eq!(deck.chord_for(fallback_keystroke(&deck, &ctrl_chord)), Some(ctrl_chord));
        // a modifier on the keystroke of a chord that already uses the thumb means two thumb buttons were pressed
        let (_, num_keystroke) = deck.keystrokes().find(|(held, _)| *held == num_chord).unwrap();
        assert_eq!(deck.chord_for(Keystroke { modifiers: num_keystroke.modifiers | 0x01, key: num_keystroke.key }), None);
        // the modifiers of two thumb buttons at once
        let (_, keystroke) = deck.keystrokes().find(|(held, _)| *held == finger_chord).unwrap();
        assert_eq!(deck.chord_for(Keystroke { modifiers: keystroke.modifiers | 0x03, key: keystroke.key }), None);

        // with the ctrl chord in the deck, the Twiddler sends its own keystroke, never the finger chord's with a modifier
        let deck = Deck::new(vec![finger_chord, ctrl_chord]).unwrap();
        assert_eq!(deck.chord_for(fallback_keystroke(&deck, &ctrl_chord)), None);
        let (_, own_keystroke) = deck.keystrokes().find(|(held, _)| *held == ctrl_chord).unwrap();
        assert_eq!(deck.chord_for(own_keystroke), Some(ctrl_chord));

        // without the finger chord in the deck, the Twiddler has nothing to send for the ctrl chord
        assert!(!Deck::new(vec![other]).unwrap().identifies(&ctrl_chord));
    }

    #[test]
    fn key_events_recorded_for_an_unmapped_ctrl_chord_identify_it() {
        // recorded from the Twiddler: ctrl with a finger button whose own output was the letter a.
        // the modifier arrives when the thumb button goes down, the letter when the chord is released.
        const KEY_LEFTCTRL: u16 = 29;
        const KEY_A: u16 = 30;
        let finger_chord = chords(1)[0];
        let deck = Deck::new(vec![finger_chord]).unwrap();
        let mut decoder = crate::decode::KeystrokeDecoder::default();
        assert_eq!(decoder.evdev_report(&[(KEY_LEFTCTRL, 1)]), Ok(None));
        let keystroke = decoder.evdev_report(&[(KEY_A, 1)]).unwrap().unwrap();
        assert_eq!(deck.chord_for(keystroke), Some(finger_chord.with_thumb(ThumbState::Ctrl)));
        assert_eq!(decoder.evdev_report(&[(KEY_LEFTCTRL, 0), (KEY_A, 0)]), Ok(None));
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
