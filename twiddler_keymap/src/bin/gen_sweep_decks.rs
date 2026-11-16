// Writes the decks for the feasibility sweep. For each deck this writes a config to load onto the Twiddler and a
// JSON file listing the deck's chords.
// - sweep_deck_0: every chord the Twiddler can see that does not use the thumb, fewest buttons first.
// - sweep_deck_thumb: a sample of the chords that do use the thumb.

use std::path::Path;

use itertools::Itertools;
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::SeedableRng;

use twiddler_keymap::chord::{Chord, FingerState, ThumbState};
use twiddler_keymap::deck::Deck;

const THUMB_SAMPLE_SEED: u64 = 0;

/// The share of the finger shapes with a given number of buttons that is tried together with a thumb button.
/// Ctrl and shift are sampled most heavily because they are the easiest thumb buttons to reach.
fn thumb_sample_fraction(thumb: ThumbState, n_finger_buttons: u32) -> f64 {
    match (thumb, n_finger_buttons) {
        (ThumbState::Lifted, _) => 0.0,
        (ThumbState::Ctrl | ThumbState::Shift, 1 | 2) => 1.0,
        (ThumbState::Ctrl | ThumbState::Shift, 3) => 0.5,
        (ThumbState::Num | ThumbState::Alt, 1) => 1.0,
        (ThumbState::Num | ThumbState::Alt, 2) => 0.25,
        _ => 0.0,
    }
}

fn uses_lowest_row(chord: &Chord) -> bool {
    chord.fingers().last() != Some(&FingerState::Lifted)
}

fn finger_chords() -> Vec<Chord> {
    let mut chords: Vec<Chord> = Chord::all().filter(|chord| chord.thumb() == ThumbState::Lifted && chord.registers_as_itself()).collect();
    // a stable sort, so that chords with equally many buttons keep a fixed order
    chords.sort_by_key(|chord| chord.n_buttons());
    chords
}

/// Chords that use the thumb, sampled separately among those that do and do not use the lowest row of finger
/// buttons, because moving the thumb up makes that row harder to reach.
fn thumb_chords(rng: &mut StdRng) -> Vec<Chord> {
    let mut chords = Vec::new();
    // the order of the thumb buttons here is the order in which the sweep asks about them
    for thumb in [ThumbState::Ctrl, ThumbState::Shift, ThumbState::Alt, ThumbState::Num] {
        let strata = finger_chords().into_iter().into_group_map_by(|chord| (chord.n_buttons(), uses_lowest_row(chord)));
        for ((n_finger_buttons, _), stratum) in strata.into_iter().sorted_by_key(|(key, _)| *key) {
            let amount = (stratum.len() as f64 * thumb_sample_fraction(thumb, n_finger_buttons)).round() as usize;
            let mut sampled: Vec<Chord> = stratum.sample(rng, amount)
                .map(|chord| Chord::new(thumb, chord.fingers()).expect("the finger buttons of a chord make a chord"))
                .collect();
            // sampling shuffles; put the chords back into a fixed order
            sampled.sort_by_key(|chord| chord.button_mask());
            chords.extend(sampled);
        }
    }
    chords
}

fn write_deck(out_dir: &Path, name: &str, chords: Vec<Chord>) {
    let n_buttons = chords.iter().map(|chord| chord.n_buttons()).minmax().into_option().expect("decks are not empty");
    let deck = Deck::new(chords).expect("the chords of a deck are distinct, registrable, and few enough for one config");
    let config_path = out_dir.join(format!("{}.cfg", name));
    let deck_path = out_dir.join(format!("{}.json", name));
    std::fs::write(&config_path, deck.config()).expect("failed to write config");
    std::fs::write(&deck_path, serde_json::to_string(&deck).expect("failed to serialize deck")).expect("failed to write deck");
    println!("{} and {}: {} chords with {} to {} buttons", config_path.display(), deck_path.display(),
             deck.chords().len(), n_buttons.0, n_buttons.1);
}

fn main() {
    let out_dir = std::env::args().nth(1).expect("usage: gen_sweep_decks <output directory>");
    let out_dir = Path::new(&out_dir);

    write_deck(out_dir, "sweep_deck_0", finger_chords());
    write_deck(out_dir, "sweep_deck_thumb", thumb_chords(&mut StdRng::seed_from_u64(THUMB_SAMPLE_SEED)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finger_chords_are_all_the_registrable_chords_without_the_thumb() {
        let chords = finger_chords();
        assert_eq!(chords.len(), 400);
        assert!(chords.is_sorted_by_key(|chord| chord.n_buttons()));
        assert!(Deck::new(chords).is_ok());
    }

    #[test]
    fn thumb_chords_follow_the_sampling_plan() {
        let chords = thumb_chords(&mut StdRng::seed_from_u64(THUMB_SAMPLE_SEED));
        // counts of registrable finger shapes with 1, 2 and 3 buttons: 12, 62 and 136
        let count = |thumb: ThumbState| chords.iter().filter(|chord| chord.thumb() == thumb).count();
        assert_eq!(count(ThumbState::Lifted), 0);
        for thumb in [ThumbState::Ctrl, ThumbState::Shift] {
            assert_eq!(count(thumb), 12 + 62 + 68);
        }
        for thumb in [ThumbState::Num, ThumbState::Alt] {
            // the two strata of 2-button shapes are rounded separately
            assert!((12 + 15..=12 + 16).contains(&count(thumb)), "{}", count(thumb));
        }
        assert!(Deck::new(chords).is_ok());
    }

    #[test]
    fn thumb_chords_cover_both_strata_of_each_sampled_size() {
        let chords = thumb_chords(&mut StdRng::seed_from_u64(THUMB_SAMPLE_SEED));
        for (thumb, n_finger_buttons) in [(ThumbState::Ctrl, 3), (ThumbState::Shift, 3), (ThumbState::Alt, 2), (ThumbState::Num, 2)] {
            for lowest_row in [false, true] {
                assert!(chords.iter().any(|chord| chord.thumb() == thumb && chord.n_buttons() == n_finger_buttons + 1 && uses_lowest_row(chord) == lowest_row),
                        "{:?} {} {}", thumb, n_finger_buttons, lowest_row);
            }
        }
    }

    #[test]
    fn thumb_chords_are_reproducible() {
        assert_eq!(thumb_chords(&mut StdRng::seed_from_u64(THUMB_SAMPLE_SEED)), thumb_chords(&mut StdRng::seed_from_u64(THUMB_SAMPLE_SEED)));
    }
}
