#![cfg(test)]
use keymap_optimization::keyboard_config::ChordSampler;
use strum::EnumCount;
use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout, TwiddlerChord};
use crate::chord_samplers::{get_possible_probabilities, MostUncertainPossibilityChordSampler, PossibleChordSampler};
use crate::train::train;
use crate::reward_model::{Ensemble, RewardEmbedding, RewardEmbeddingBase, RewardModel};

const TEST_RESULTS_PATH: &str = "./src/tests/test_data";

fn train_and_sample<E: RewardEmbedding>(quality_ratio: f64, n_epochs: usize, data_path: &str) {
    let model = match train::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, E>(data_path, n_epochs) {
        Ok(model) => model,
        Err(e) => return assert!(false, "Error training model: {}", e)
    };

    let mut chords_with_probs = match get_possible_probabilities::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, E>(&Box::new(model.chord_embedding)) {
        Ok(chords_with_probs) => chords_with_probs,
        Err(e) => return assert!(false, "Error training model: {}", e)
    };

    chords_with_probs.sort_by(|(_, p1), (_, p2)| p1.partial_cmp(p2).unwrap());

    // print the chords with the highest and lowest probabilities of being possible
    const N_AVG: usize = 20;

    let highest_prob_poss:  Vec<&(TwiddlerChord, f64)> = chords_with_probs.iter().rev().take(N_AVG).collect();
    let lowest_prob_poss: Vec<&(TwiddlerChord, f64)> = chords_with_probs.iter().take(N_AVG).collect();

    // print the most and least likely possible chords
    println!("most likely possible chords:");
    for (chord, prob) in highest_prob_poss.iter() {
        println!("{:<25}: {}", chord, prob);
    }
    println!("least likely possible chords:");
    for (chord, prob) in lowest_prob_poss.iter() {
        println!("{:<25}: {}", chord, prob);
    }

    // assert that the average number of keys pressed for the most-likely-possible chords is
    // meaningfully lower than that for the least-likely-possible chords, as a sanity check

    let avg_n_keys_low = highest_prob_poss.into_iter().map(|(chord, _)| chord.n_keys()).sum::<usize>() as f64 / (N_AVG as f64);
    let avg_n_keys_high = lowest_prob_poss.into_iter().map(|(chord, _)| chord.n_keys()).sum::<usize>() as f64 / (N_AVG as f64);

    println!("average number of keys pressed for most-likely-possible chords vs least-likely-possible chords: {} vs {}", avg_n_keys_low, avg_n_keys_high);

    assert!(avg_n_keys_high > quality_ratio * avg_n_keys_low, "average number of keys pressed for most-likely-possible chords ({}) is not substantially less than that for least-likely-possible chords ({})", avg_n_keys_low, avg_n_keys_high);
}

#[test]
fn train_and_sample_single() {
    train_and_sample::<RewardEmbeddingBase<{ TwiddlerKey::COUNT }>>(1.2, 1001, TEST_RESULTS_PATH);
}

#[test]
fn train_and_sample_ensemble() {
    train_and_sample::<Ensemble<RewardModel<{ TwiddlerKey::COUNT }, RewardEmbeddingBase<{ TwiddlerKey::COUNT }>>>>(2.0, 501, TEST_RESULTS_PATH);
}

fn test_sampler<I, S: ChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, rand::rngs::ThreadRng, I>>(initialization_info: &I) {
    let mut sampler = match S::new(rand::thread_rng(), initialization_info) {
        Ok(s) => s,
        Err(e) => return assert!(false, "Error creating sampler: {}", e)
    };
    println!();
    println!("sampled chords:");
    for _ in 0..10 {
        let chord = <S as ChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, rand::rngs::ThreadRng, I>>::sample_chord(&mut sampler);
        println!("{}", chord);
    }
}

#[test]
fn test_exponential_sampler() {
    test_sampler::<(), keymap_optimization::twiddler::TwiddlerExponentialSampler<rand::rngs::ThreadRng>>(&());
}

#[test]
fn test_slow_samplers() {
    type E = RewardEmbeddingBase<{ TwiddlerKey::COUNT }>;
    // since we're just checking that nothing panics, we can train the model for a very short time since its performance doesn't matter
    let embedder = match train::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, E>(TEST_RESULTS_PATH, 101) {
        Ok(model) => Box::new(model.chord_embedding),
        Err(e) => return assert!(false, "Error training model: {}", e)
    };

    test_sampler::<E, MostUncertainPossibilityChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, rand::rngs::ThreadRng>>(&embedder);
    test_sampler::<E, PossibleChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, rand::rngs::ThreadRng>>(&embedder);
}
