use keymap_optimization::keyboard_config::{Chord, Key, Layout, ChordSampler};
use itertools::Itertools;
use tch::Tensor;
use crate::train::{chord_to_tensor, train};
use rand::prelude::SliceRandom;

fn all_chords<K: Key, const N: usize, L: Layout<K, N>>() -> Vec<Chord<K, N, L>> {
    // generate all 2^16 = 65536 chords and return the valid ones
    let mut chords = Vec::new();
    for keys in K::VARIANTS.iter().powerset() {
        let mut chord = Chord::new();
        for key in keys.iter() {
            chord.add_key(**key);
        }
        if L::is_valid(&chord) {
            chords.push(chord);
        }
    }
    chords
}

pub fn get_impossible_probabilities<K: Key, const N: usize, L: Layout<K, N>>(results_path: &str) -> Result<Vec<(Chord<K, N, L>, f64)>, Box<dyn std::error::Error>> {
    let all_chords: Vec<Chord<K, N, L>> = all_chords::<K, N, L>();
    let all_chords_tensor = Tensor::stack(&all_chords.clone().into_iter().map(|c| chord_to_tensor(&c)).collect::<Vec<Tensor>>(), 0);

    let model = match train::<K, N, L>(results_path) {
        Ok(model) => model,
        Err(e) => return Err(e),
    };

    let embedder = model.chord_embedding;

    // compute the probability of being impossible for each chord
    let (_, _, impossible_probs) = embedder.forward(&all_chords_tensor);

    Ok(all_chords.into_iter()
                 .zip(impossible_probs.squeeze()
                                      .iter::<f64>()
                                      .unwrap())
                 .collect())
}

pub struct PossibleChordSampler<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng> {
    rng: R,
    chords_with_impossible_probs: Vec<(Chord<K, N, L>, f64)>,
}

impl<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng> ChordSampler<K, N, L, R, &str> for PossibleChordSampler<K, N, L, R> {
    fn new(rng: R, results_path: Box<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let chords_with_impossible_probs = match get_impossible_probabilities::<K, N, L>(&results_path) {
            Ok(chords_with_probs) => chords_with_probs,
            Err(e) => return Err(e),
        };
        Ok(Self { rng, chords_with_impossible_probs })
    }

    fn sample_chord(&mut self) -> Chord<K, N, L> {
        // sample chords weighted towards those that are more likely to be possible

        // rejection sample chords based on their probability of being impossible
        loop {
            // select a random element of impossible_probs
            let (chord, impossible_prob) = self.chords_with_impossible_probs.choose(&mut rand::thread_rng()).unwrap();  // unwrap is safe because there are always chords
            if self.rng.gen::<f64>() < 1.0 - impossible_prob {
                return chord.clone()
            }
        }
    }
}
