#![cfg(test)]
use strum::EnumCount;
use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout, TwiddlerChord};
use crate::possible_chord_sampler::get_impossible_probabilities;

#[test]
pub fn train_and_sample() {
    const TEST_RESULTS_PATH: &str = "./src/tests/test_data";

    let mut chords_with_probs = match get_impossible_probabilities::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>(TEST_RESULTS_PATH) {
        Ok(chords_with_probs) => chords_with_probs,
        Err(e) => return assert!(false, "Error training model: {}", e)
    };
    chords_with_probs.sort_by(|(_, p1), (_, p2)| p1.partial_cmp(p2).unwrap());

    // print the chords with the highest and lowest probabilities of being impossible
    const N_AVG: usize = 20;

    let highest_prob_imp: Vec<&(TwiddlerChord, f64)> =  chords_with_probs.iter().rev().take(N_AVG).collect();
    let lowest_prob_imp: Vec<&(TwiddlerChord, f64)> = chords_with_probs.iter().take(N_AVG).collect();

    // print the most and least likely impossible chords
    println!("most likely impossible chords:");
    for (chord, prob) in highest_prob_imp.iter() {
        println!("{:<25}: {}", chord, prob);
    }
    println!("least likely impossible chords:");
    for (chord, prob) in lowest_prob_imp.iter() {
        println!("{:<25}: {}", chord, prob);
    }

    // assert that the average number of keys pressed for the most-likely-impossible chords is higher than that for the least-likely-impossible chords,
    // as a sanity check

    let avg_n_keys_high = highest_prob_imp.into_iter().map(|(chord, _)| chord.n_keys()).sum::<usize>() as f64 / (N_AVG as f64);
    let avg_n_keys_low = lowest_prob_imp.into_iter().map(|(chord, _)| chord.n_keys()).sum::<usize>() as f64 / (N_AVG as f64);

    println!("average number of keys pressed for most-likely-impossible chords vs least-likely-impossible chords: {} vs {}", avg_n_keys_high, avg_n_keys_low);

    assert!(avg_n_keys_high > avg_n_keys_low, "average number of keys pressed for most-likely-impossible chords ({}) is not greater than that for least-likely-impossible chords ({})", avg_n_keys_high, avg_n_keys_low);
}