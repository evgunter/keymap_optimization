//! The feasibility sweep: going through a deck and recording, for each chord, whether it can be pressed and how
//! comfortable it is.
//!
//! A chord counts as pressed when the Twiddler sees it, however it was pressed (see `Chord::registers_as`),
//! so an outcome describes the easiest way of pressing the chord.

use std::collections::HashMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::chord::Chord;
use crate::deck::Deck;
use crate::keystroke::Keystroke;

/// How comfortable a chord is to press, from best to worst.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Comfort {
    Comfortable,
    Ok,
    Awkward,
    Painful,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Outcome {
    /// The chord was pressed and the Twiddler sent its keystroke.
    Pressed(Comfort),
    /// The hand cannot form the chord, in any of the ways of pressing it.
    CannotPress,
    /// The chord was pressed, but the Twiddler did not send its keystroke.
    NotRegistered,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Record {
    pub chord: Chord,
    pub outcome: Outcome,
    pub time: SystemTime,
}

/// The outcome of each chord in the records. If a chord has several records, the last one counts.
pub fn outcomes(records: &[Record]) -> HashMap<Chord, Outcome> {
    records.iter().map(|record| (record.chord, record.outcome)).collect()
}

/// What the session is waiting for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// The chord should be pressed, or reported as not pressable.
    Press(Chord),
    /// The chord was pressed and should be rated.
    Rate(Chord),
    /// Every chord in the deck is dealt with.
    Finished,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    CannotPress,
    NotRegistered,
    Rate(Comfort),
    /// Go back: from rating to pressing the same chord, or from pressing to the chord recorded before it.
    Undo,
}

/// How the session reacted to an input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Response {
    /// The step changed without anything being recorded.
    Moved,
    /// The outcome was recorded and should be saved.
    Recorded(Chord, Outcome),
    /// The Twiddler sent the keystroke of a different chord of the deck, or of none.
    WrongChord(Option<Chord>),
    NothingToUndo,
    /// The input has no meaning at the current step.
    Ignored,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Progress {
    pub recorded: usize,
    /// Chords left out because every way of pressing them includes a way of pressing a chord that cannot be pressed.
    pub skipped: usize,
    pub remaining: usize,
}

/// One sitting of the sweep over one deck.
pub struct Session {
    deck: Deck,
    /// The outcomes known so far, including those of chords outside the deck.
    outcomes: HashMap<Chord, Outcome>,
    /// Whether to skip chords that cannot be pressed without pressing a chord already found impossible.
    skip_supersets: bool,
    fingerings: HashMap<Chord, Vec<Chord>>,
    /// The chords recorded in this session, oldest first.
    history: Vec<Chord>,
    step: Step,
}

impl Session {
    pub fn new(deck: Deck, outcomes: HashMap<Chord, Outcome>, skip_supersets: bool) -> Self {
        let mut session = Session { deck, outcomes, skip_supersets, fingerings: Chord::fingerings(), history: Vec::new(), step: Step::Finished };
        session.step = session.next_step();
        session
    }

    pub fn step(&self) -> Step {
        self.step
    }

    /// The ways of pressing a chord, including ones the Twiddler sees as a different chord.
    fn ways_of_pressing<'a>(&'a self, chord: &'a Chord) -> &'a [Chord] {
        // a chord that registers as another one is not among the keys; the only way of pressing it is itself
        self.fingerings.get(chord).map_or(std::slice::from_ref(chord), Vec::as_slice)
    }

    /// The hand shapes known to be impossible: every way of pressing a chord that cannot be pressed.
    fn impossible_shapes(&self) -> Vec<Chord> {
        self.outcomes.iter()
            .filter(|(_, outcome)| **outcome == Outcome::CannotPress)
            .flat_map(|(chord, _)| self.ways_of_pressing(chord))
            .copied()
            .collect()
    }

    fn is_skipped(&self, chord: &Chord, impossible_shapes: &[Chord]) -> bool {
        self.skip_supersets && self.ways_of_pressing(chord).iter().all(|way| impossible_shapes.iter().any(|shape| way.contains(shape)))
    }

    fn next_step(&self) -> Step {
        let impossible_shapes = self.impossible_shapes();
        self.deck.chords().iter()
            .find(|chord| !self.outcomes.contains_key(chord) && !self.is_skipped(chord, &impossible_shapes))
            .map_or(Step::Finished, |chord| Step::Press(*chord))
    }

    pub fn progress(&self) -> Progress {
        let impossible_shapes = self.impossible_shapes();
        let unrecorded: Vec<&Chord> = self.deck.chords().iter().filter(|chord| !self.outcomes.contains_key(chord)).collect();
        let skipped = unrecorded.iter().filter(|chord| self.is_skipped(chord, &impossible_shapes)).count();
        Progress { recorded: self.deck.chords().len() - unrecorded.len(), skipped, remaining: unrecorded.len() - skipped }
    }

    fn record(&mut self, chord: Chord, outcome: Outcome) -> Response {
        self.outcomes.insert(chord, outcome);
        self.history.push(chord);
        self.step = self.next_step();
        Response::Recorded(chord, outcome)
    }

    pub fn handle_keystroke(&mut self, keystroke: Keystroke) -> Response {
        match self.step {
            Step::Press(target) => match self.deck.chord_for(keystroke) {
                Some(chord) if chord == target => {
                    self.step = Step::Rate(target);
                    Response::Moved
                }
                other => Response::WrongChord(other),
            },
            Step::Rate(_) | Step::Finished => Response::Ignored,
        }
    }

    pub fn handle_command(&mut self, command: Command) -> Response {
        match (self.step, command) {
            (Step::Press(chord), Command::CannotPress) => self.record(chord, Outcome::CannotPress),
            (Step::Press(chord), Command::NotRegistered) => self.record(chord, Outcome::NotRegistered),
            (Step::Rate(chord), Command::Rate(comfort)) => self.record(chord, Outcome::Pressed(comfort)),
            (Step::Rate(chord), Command::Undo) => {
                self.step = Step::Press(chord);
                Response::Moved
            }
            (Step::Press(_) | Step::Finished, Command::Undo) => match self.history.pop() {
                Some(chord) => {
                    // the saved record stays in the results until the chord is recorded again, which replaces it
                    self.outcomes.remove(&chord);
                    self.step = Step::Press(chord);
                    Response::Moved
                }
                None => Response::NothingToUndo,
            },
            (Step::Press(_), Command::Rate(_))
            | (Step::Rate(_), Command::CannotPress | Command::NotRegistered)
            | (Step::Finished, Command::CannotPress | Command::NotRegistered | Command::Rate(_)) => Response::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::{FingerState::{self, Lifted, L, LM, M}, ThumbState};

    fn chord(fingers: [FingerState; 4]) -> Chord {
        Chord::new(ThumbState::Lifted, fingers).unwrap()
    }

    // a deck whose third chord contains its first, and the keystrokes its chords send
    fn deck() -> (Deck, [Chord; 3], [Keystroke; 3]) {
        let chords = [chord([L, Lifted, Lifted, Lifted]), chord([M, Lifted, Lifted, Lifted]), chord([LM, Lifted, Lifted, Lifted])];
        (Deck::new(chords.to_vec()).unwrap(), chords, [0, 1, 2].map(|index| Keystroke::from_index(index).unwrap()))
    }

    #[test]
    fn pressing_and_rating_each_chord_finishes_the_deck() {
        let (deck, chords, keystrokes) = deck();
        let mut session = Session::new(deck, HashMap::new(), true);
        for (chord, keystroke) in chords.into_iter().zip(keystrokes) {
            assert_eq!(session.step(), Step::Press(chord));
            assert_eq!(session.handle_keystroke(keystroke), Response::Moved);
            assert_eq!(session.step(), Step::Rate(chord));
            assert_eq!(session.handle_command(Command::Rate(Comfort::Ok)), Response::Recorded(chord, Outcome::Pressed(Comfort::Ok)));
        }
        assert_eq!(session.step(), Step::Finished);
        assert_eq!(session.progress(), Progress { recorded: 3, skipped: 0, remaining: 0 });
    }

    #[test]
    fn wrong_keystrokes_are_reported_and_change_nothing() {
        let (deck, chords, keystrokes) = deck();
        let mut session = Session::new(deck, HashMap::new(), true);
        assert_eq!(session.handle_keystroke(keystrokes[1]), Response::WrongChord(Some(chords[1])));
        assert_eq!(session.handle_keystroke(Keystroke::from_index(3).unwrap()), Response::WrongChord(None));
        assert_eq!(session.step(), Step::Press(chords[0]));
    }

    #[test]
    fn chords_containing_one_that_cannot_be_pressed_are_skipped() {
        let (deck, chords, _) = deck();
        let mut session = Session::new(deck, HashMap::new(), true);
        assert_eq!(session.handle_command(Command::CannotPress), Response::Recorded(chords[0], Outcome::CannotPress));
        assert_eq!(session.step(), Step::Press(chords[1]));
        assert_eq!(session.handle_command(Command::NotRegistered), Response::Recorded(chords[1], Outcome::NotRegistered));
        assert_eq!(session.step(), Step::Finished);
        assert_eq!(session.progress(), Progress { recorded: 2, skipped: 1, remaining: 0 });
    }

    #[test]
    fn a_chord_is_skipped_only_if_every_way_of_pressing_it_is_impossible() {
        let square = chord([LM, LM, Lifted, Lifted]);
        let deck = Deck::new(vec![square]).unwrap();
        let skipped = |impossible: &[Chord]| {
            let outcomes = impossible.iter().map(|chord| (*chord, Outcome::CannotPress)).collect();
            Session::new(deck.clone(), outcomes, true).step() == Step::Finished
        };

        // the square can still be pressed as [LM, M], which does not include [L, L]
        assert!(!skipped(&[chord([L, L, Lifted, Lifted])]));
        // a chord the Twiddler sees as another one stands for that hand shape only
        let three_corners = [chord([LM, L, Lifted, Lifted]), chord([LM, M, Lifted, Lifted]), chord([L, LM, Lifted, Lifted])];
        assert!(!skipped(&three_corners));
        // every way of pressing the square includes one of these
        assert!(skipped(&[three_corners[0], three_corners[1], three_corners[2], chord([M, LM, Lifted, Lifted])]));
        assert!(skipped(&[chord([L, L, Lifted, Lifted]), chord([M, M, Lifted, Lifted])]));
    }

    #[test]
    fn skipping_can_be_turned_off() {
        let (deck, chords, _) = deck();
        let outcomes = HashMap::from([(chords[0], Outcome::CannotPress), (chords[1], Outcome::CannotPress)]);
        let session = Session::new(deck, outcomes, false);
        assert_eq!(session.step(), Step::Press(chords[2]));
        assert_eq!(session.progress(), Progress { recorded: 2, skipped: 0, remaining: 1 });
    }

    #[test]
    fn earlier_outcomes_are_not_asked_again_and_cause_skips_even_from_other_decks() {
        let (_, chords, _) = deck();
        let deck = Deck::new(chords[1..].to_vec()).unwrap();
        let session = Session::new(deck, HashMap::from([(chords[0], Outcome::CannotPress)]), true);
        assert_eq!(session.step(), Step::Press(chords[1]));
        assert_eq!(session.progress(), Progress { recorded: 0, skipped: 1, remaining: 1 });
    }

    #[test]
    fn undo_while_rating_returns_to_pressing_the_same_chord() {
        let (deck, chords, keystrokes) = deck();
        let mut session = Session::new(deck, HashMap::new(), true);
        session.handle_keystroke(keystrokes[0]);
        assert_eq!(session.handle_command(Command::Undo), Response::Moved);
        assert_eq!(session.step(), Step::Press(chords[0]));
    }

    #[test]
    fn undo_while_pressing_returns_to_the_previous_chord() {
        let (deck, chords, _) = deck();
        let mut session = Session::new(deck, HashMap::new(), true);
        assert_eq!(session.handle_command(Command::Undo), Response::NothingToUndo);
        session.handle_command(Command::CannotPress);
        session.handle_command(Command::NotRegistered);
        assert_eq!(session.step(), Step::Finished);

        assert_eq!(session.handle_command(Command::Undo), Response::Moved);
        assert_eq!(session.step(), Step::Press(chords[1]));
        assert_eq!(session.handle_command(Command::Undo), Response::Moved);
        assert_eq!(session.step(), Step::Press(chords[0]));
        assert_eq!(session.handle_command(Command::Undo), Response::NothingToUndo);
        // with the first chord no longer marked as impossible, the third is no longer skipped
        assert_eq!(session.progress(), Progress { recorded: 0, skipped: 0, remaining: 3 });
    }

    #[test]
    fn inputs_that_do_not_apply_are_ignored() {
        let (deck, chords, keystrokes) = deck();
        let mut session = Session::new(deck, HashMap::new(), true);
        assert_eq!(session.handle_command(Command::Rate(Comfort::Painful)), Response::Ignored);
        session.handle_keystroke(keystrokes[0]);
        assert_eq!(session.handle_command(Command::CannotPress), Response::Ignored);
        assert_eq!(session.handle_command(Command::NotRegistered), Response::Ignored);
        assert_eq!(session.handle_keystroke(keystrokes[0]), Response::Ignored);
        assert_eq!(session.step(), Step::Rate(chords[0]));
    }

    #[test]
    fn later_records_replace_earlier_ones() {
        let (_, chords, _) = deck();
        let records = [(chords[0], Outcome::CannotPress), (chords[1], Outcome::NotRegistered), (chords[0], Outcome::Pressed(Comfort::Awkward))]
            .map(|(chord, outcome)| Record { chord, outcome, time: SystemTime::now() });
        assert_eq!(outcomes(&records), HashMap::from([(chords[0], Outcome::Pressed(Comfort::Awkward)), (chords[1], Outcome::NotRegistered)]));
    }

    #[test]
    fn records_survive_a_results_file() {
        let (_, chords, _) = deck();
        let record = Record { chord: chords[0], outcome: Outcome::Pressed(Comfort::Ok), time: SystemTime::now() };
        let mut results = Vec::new();
        crate::results::append_record(&mut results, &record).unwrap();
        assert_eq!(crate::results::read_records::<Record>(results.as_slice()).unwrap(), [record]);
    }
}
