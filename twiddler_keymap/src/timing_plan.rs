//! Chooses which pairs of chords to time, from the results of the feasibility sweep.
//!
//! Each chord worth timing is alternated with a reference chord: one of a few simple chords that every other
//! chord is compared through. Every so often a pair is repeated, to show how much times vary between
//! measurements and how they drift as the session goes on.

use std::collections::HashMap;

use itertools::Itertools;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, RngExt};

use crate::chord::{Chord, FingerState, ThumbState, N_FINGERS};
use crate::deck::MAX_CHORDS;
use crate::sweep::{Comfort, Outcome};

/// One trial in this many is a repeat.
pub const REPEAT_EVERY: usize = 10;
/// A repeated trial comes at least this many trials after the trial it repeats.
const MIN_REPEAT_GAP: usize = 20;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum PlanError {
    #[error("the sweep has no pressable single-button chord for finger {0}, so it has no reference chord")]
    NoReference(usize),
}

fn comfort(outcomes: &HashMap<Chord, Outcome>, chord: &Chord) -> Option<Comfort> {
    match outcomes.get(chord) {
        Some(Outcome::Pressed(comfort)) => Some(*comfort),
        Some(Outcome::CannotPress | Outcome::NotRegistered) | None => None,
    }
}

/// The chords worth timing: those that were pressed in the sweep and are not painful.
pub fn candidates(outcomes: &HashMap<Chord, Outcome>) -> Vec<Chord> {
    outcomes.keys().copied()
        .filter(|chord| comfort(outcomes, chord).is_some_and(|comfort| comfort < Comfort::Painful))
        // a fixed order, since the order of a hash map changes from run to run
        .sorted_by_key(|chord| chord.button_mask())
        .collect()
}

/// For each finger, its most comfortable single-button chord, preferring the middle button among equals.
pub fn references(outcomes: &HashMap<Chord, Outcome>) -> Result<[Chord; N_FINGERS], PlanError> {
    let mut references = Vec::new();
    for finger in 0..N_FINGERS {
        let reference = [FingerState::M, FingerState::L, FingerState::R].into_iter()
            .map(|state| {
                let mut fingers = [FingerState::Lifted; N_FINGERS];
                fingers[finger] = state;
                Chord::new(ThumbState::Lifted, fingers).expect("the chord presses a finger button")
            })
            .filter(|chord| comfort(outcomes, chord).is_some())
            // of equally comfortable chords, min_by_key keeps the first
            .min_by_key(|chord| comfort(outcomes, chord))
            .ok_or(PlanError::NoReference(finger))?;
        references.push(reference);
    }
    Ok(references.try_into().expect("one reference was chosen per finger"))
}

/// One trial per candidate, in random order: the candidate with a reference other than itself. The references
/// take turns, so each is used about equally often. Which chord of a pair comes first is random.
fn base_trials(candidates: &[Chord], references: &[Chord; N_FINGERS], rng: &mut impl Rng) -> Vec<[Chord; 2]> {
    let mut shuffled = candidates.to_vec();
    shuffled.shuffle(rng);
    shuffled.into_iter().enumerate().map(|(index, chord)| {
        let reference = (0..N_FINGERS).map(|offset| references[(index + offset) % N_FINGERS])
            .find(|reference| *reference != chord)
            .expect("at most one of the distinct references is the chord itself");
        if rng.random() { [chord, reference] } else { [reference, chord] }
    }).collect()
}

fn same_pair(a: &[Chord; 2], b: &[Chord; 2]) -> bool {
    a == b || (a[0] == b[1] && a[1] == b[0])
}

/// Inserts repeats so that one trial in `REPEAT_EVERY` is a repeat. Repeats alternate between an anchor (the
/// pairs of references, in rotation) and a random earlier trial; the first few are all anchors, until there
/// are trials far enough back to repeat. A repeat is never next to a trial with the same two chords.
fn with_repeats(base: &[[Chord; 2]], references: &[Chord; N_FINGERS], rng: &mut impl Rng) -> Vec<[Chord; 2]> {
    let anchors: Vec<[Chord; 2]> = references.iter().copied().array_combinations().collect();
    let mut plan = Vec::new();
    let (mut n_repeats, mut next_anchor) = (0, 0);
    for (index, pair) in base.iter().enumerate() {
        plan.push(*pair);
        if (index + 1).is_multiple_of(REPEAT_EVERY - 1) {
            // the trials at least MIN_REPEAT_GAP before the repeat, which follows trial `index`
            let far_enough_back = &base[..(index + 2).saturating_sub(MIN_REPEAT_GAP)];
            let earlier = if n_repeats % 2 == 1 { far_enough_back.choose(rng) } else { None };
            plan.push(*earlier.unwrap_or_else(|| {
                // two trials are next to the repeat and they cannot both rule out more than two anchors
                let neighbours = [Some(pair), base.get(index + 1)];
                let anchor = (0..anchors.len()).map(|offset| &anchors[(next_anchor + offset) % anchors.len()])
                    .find(|anchor| !neighbours.iter().flatten().any(|neighbour| same_pair(anchor, neighbour)))
                    .expect("there are more anchors than neighbouring trials");
                next_anchor += 1;
                anchor
            }));
            n_repeats += 1;
        }
    }
    plan
}

/// The pairs to time, in order.
pub fn plan(outcomes: &HashMap<Chord, Outcome>, rng: &mut impl Rng) -> Result<Vec<[Chord; 2]>, PlanError> {
    let references = references(outcomes)?;
    let base = base_trials(&candidates(outcomes), &references, rng);
    Ok(with_repeats(&base, &references, rng))
}

/// The chords for the deck used with a plan: the plan's chords, and as many others as fit, fewest buttons first.
/// The Twiddler sends nothing for a chord outside the deck, so the others are there to let a mistaken chord be
/// recognised and shown.
pub fn deck_chords(plan: &[[Chord; 2]]) -> Vec<Chord> {
    let by_size = |chord: &Chord| (chord.n_buttons(), chord.button_mask());
    let planned: Vec<Chord> = plan.iter().flatten().copied().unique().collect();
    let others = Chord::all()
        .filter(|chord| chord.registers_as_itself() && !planned.contains(chord))
        .sorted_by_key(by_size)
        .take(MAX_CHORDS.saturating_sub(planned.len()));
    planned.iter().copied().chain(others).sorted_by_key(by_size).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use FingerState::{Lifted, L, M, R};

    fn chord(fingers: [FingerState; N_FINGERS]) -> Chord {
        Chord::new(ThumbState::Lifted, fingers).unwrap()
    }

    /// Every registrable chord without the thumb, pressed with the same comfort.
    fn sweep(comfort: Comfort) -> HashMap<Chord, Outcome> {
        Chord::all().filter(|chord| chord.thumb() == ThumbState::Lifted && chord.registers_as_itself())
            .map(|chord| (chord, Outcome::Pressed(comfort)))
            .collect()
    }

    fn middle_buttons() -> [Chord; N_FINGERS] {
        [chord([M, Lifted, Lifted, Lifted]), chord([Lifted, M, Lifted, Lifted]), chord([Lifted, Lifted, M, Lifted]), chord([Lifted, Lifted, Lifted, M])]
    }

    #[test]
    fn candidates_are_the_chords_pressed_without_pain() {
        let mut outcomes = sweep(Comfort::Awkward);
        let [painful, impossible, unregistered, _] = middle_buttons();
        outcomes.insert(painful, Outcome::Pressed(Comfort::Painful));
        outcomes.insert(impossible, Outcome::CannotPress);
        outcomes.insert(unregistered, Outcome::NotRegistered);
        let candidates = candidates(&outcomes);
        assert_eq!(candidates.len(), 400 - 3);
        assert!([painful, impossible, unregistered].iter().all(|chord| !candidates.contains(chord)));
        assert!(candidates.is_sorted_by_key(|chord| chord.button_mask()));
    }

    #[test]
    fn references_are_the_most_comfortable_single_buttons_preferring_the_middle() {
        let mut outcomes = sweep(Comfort::Ok);
        assert_eq!(references(&outcomes), Ok(middle_buttons()));

        // a more comfortable button wins over the middle one, and a button that was not pressed is never chosen
        outcomes.insert(chord([R, Lifted, Lifted, Lifted]), Outcome::Pressed(Comfort::Comfortable));
        outcomes.insert(chord([Lifted, M, Lifted, Lifted]), Outcome::CannotPress);
        let chosen = references(&outcomes).unwrap();
        assert_eq!(chosen[0], chord([R, Lifted, Lifted, Lifted]));
        assert_eq!(chosen[1], chord([Lifted, L, Lifted, Lifted]));
    }

    #[test]
    fn a_finger_without_a_pressable_single_button_is_an_error() {
        let mut outcomes = sweep(Comfort::Ok);
        for state in [L, M, R] {
            outcomes.remove(&chord([Lifted, Lifted, state, Lifted]));
        }
        assert_eq!(references(&outcomes), Err(PlanError::NoReference(2)));
    }

    #[test]
    fn base_trials_pair_each_candidate_once_with_evenly_used_references() {
        let outcomes = sweep(Comfort::Ok);
        let (candidates, references) = (candidates(&outcomes), middle_buttons());
        let base = base_trials(&candidates, &references, &mut StdRng::seed_from_u64(0));
        assert_eq!(base.len(), candidates.len());

        // each trial is a reference and a different chord; taking the other chord of each trial gives each candidate once
        let mut reference_uses = HashMap::new();
        let mut others = Vec::new();
        for pair in &base {
            assert_ne!(pair[0], pair[1]);
            // when both chords are references, either could be the one standing in as reference
            let reference_position = pair.iter().position(|chord| references.contains(chord)).expect("a trial includes a reference");
            *reference_uses.entry(pair[reference_position]).or_insert(0) += 1;
            others.push(pair[1 - reference_position]);
        }
        let reference_pairs = base.iter().filter(|pair| pair.iter().all(|chord| references.contains(chord))).count();
        assert_eq!(reference_pairs, N_FINGERS);
        assert!(others.iter().filter(|chord| !references.contains(chord)).all_unique());
        assert_eq!(others.iter().filter(|chord| !references.contains(chord)).count(), candidates.len() - N_FINGERS);
        assert!(reference_uses.values().all(|uses| (*uses as i64 - 100).abs() <= 2 * N_FINGERS as i64), "{:?}", reference_uses.values());
        // both orders occur
        assert!(base.iter().any(|pair| references.contains(&pair[0])) && base.iter().any(|pair| !references.contains(&pair[0])));
    }

    #[test]
    fn repeats_are_anchors_or_earlier_trials_far_enough_back() {
        let outcomes = sweep(Comfort::Ok);
        let references = middle_buttons();
        let base = base_trials(&candidates(&outcomes), &references, &mut StdRng::seed_from_u64(0));
        let plan = with_repeats(&base, &references, &mut StdRng::seed_from_u64(1));
        assert_eq!(plan.len(), base.len() + base.len() / (REPEAT_EVERY - 1));

        let is_repeat = |position: usize| (position + 1).is_multiple_of(REPEAT_EVERY);
        let without_repeats: Vec<[Chord; 2]> = plan.iter().enumerate().filter(|(position, _)| !is_repeat(*position)).map(|(_, pair)| *pair).collect();
        assert_eq!(without_repeats, base);

        let (mut n_anchors, mut n_earlier) = (0, 0);
        for position in (0..plan.len()).filter(|position| is_repeat(*position)) {
            let is_anchor = plan[position].iter().all(|chord| references.contains(chord));
            let repeats_far_back = position >= MIN_REPEAT_GAP && plan[..=position - MIN_REPEAT_GAP].contains(&plan[position]);
            assert!(is_anchor || repeats_far_back, "repeat at {}", position);
            if is_anchor { n_anchors += 1 } else { n_earlier += 1 }
        }
        assert!(n_anchors >= n_earlier && n_earlier >= n_anchors - 4, "{} anchors, {} earlier trials", n_anchors, n_earlier);
        // every pair of references serves as an anchor
        let anchors_used = plan.iter().enumerate().filter(|(position, pair)| is_repeat(*position) && pair.iter().all(|chord| references.contains(chord))).map(|(_, pair)| *pair).unique().count();
        assert_eq!(anchors_used, 6);
    }

    #[test]
    fn the_same_pair_is_never_timed_twice_in_a_row() {
        let outcomes = sweep(Comfort::Ok);
        for seed in 0..200 {
            let plan = plan(&outcomes, &mut StdRng::seed_from_u64(seed)).unwrap();
            assert!(plan.iter().tuple_windows().all(|(earlier, later)| !same_pair(earlier, later)), "seed {}", seed);
        }
    }

    #[test]
    fn an_anchor_is_passed_over_when_a_neighbouring_trial_has_the_same_chords() {
        let references = middle_buttons();
        let [a, b, c, d] = references;
        // the anchors in rotation start [a, b], [a, c]; the trial before the first repeat already times a and b
        let mut base = vec![[a, d]; REPEAT_EVERY - 1];
        base[REPEAT_EVERY - 2] = [b, a];
        let plan = with_repeats(&base, &references, &mut StdRng::seed_from_u64(0));
        assert_eq!(plan[REPEAT_EVERY - 1], [a, c]);
    }

    #[test]
    fn the_deck_holds_the_plan_and_is_filled_with_the_smallest_other_chords() {
        let [a, b, _, _] = middle_buttons();
        let large = chord([FingerState::LMR, FingerState::LMR, FingerState::LMR, FingerState::LMR]);
        let chords = deck_chords(&[[a, b], [large, a]]);
        assert_eq!(chords.len(), MAX_CHORDS);
        assert!(crate::deck::Deck::new(chords.clone()).is_ok());
        assert!([a, b, large].iter().all(|chord| chords.contains(chord)));

        // every chord left out has at least as many buttons as every chord added to the plan's
        let largest_added = chords.iter().filter(|chord| **chord != large).map(|chord| chord.n_buttons()).max().unwrap();
        let smallest_left_out = Chord::all().filter(|chord| chord.registers_as_itself() && !chords.contains(chord)).map(|chord| chord.n_buttons()).min().unwrap();
        assert!(smallest_left_out >= largest_added, "{} left out, {} added", smallest_left_out, largest_added);
    }

    #[test]
    fn plans_are_reproducible() {
        let outcomes = sweep(Comfort::Ok);
        assert_eq!(plan(&outcomes, &mut StdRng::seed_from_u64(7)), plan(&outcomes, &mut StdRng::seed_from_u64(7)));
        assert_ne!(plan(&outcomes, &mut StdRng::seed_from_u64(7)), plan(&outcomes, &mut StdRng::seed_from_u64(8)));
    }
}
