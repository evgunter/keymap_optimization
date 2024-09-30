use keymap_optimization::keyboard_config::{Chord, Key, Layout, ChordSampler};
use itertools::Itertools;
use tch::Tensor;
use crate::train::chord_to_tensor;
use crate::reward_model::RewardEmbedding;
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

pub fn get_possible_probabilities<K: Key, const N: usize, L: Layout<K, N>, E: RewardEmbedding>(embedder: &E) -> Result<Vec<(Chord<K, N, L>, f64)>, Box<dyn std::error::Error>> {
    let all_chords: Vec<Chord<K, N, L>> = all_chords::<K, N, L>();
    let all_chords_tensor = Tensor::stack(&all_chords.clone().into_iter().map(|c| chord_to_tensor(&c)).collect::<Vec<Tensor>>(), 0);

    // compute the probability of being possible for each chord
    let (_, _, possible_probs) = embedder.embed_chords(&all_chords_tensor);

    Ok(all_chords.into_iter()
                 .zip(possible_probs.squeeze()
                                    .iter::<f64>()
                                    .unwrap())
                 .collect())
}

pub struct PossibleChordSampler<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng> {
    rng: R,
    chords_with_possible_probs: Vec<(Chord<K, N, L>, f64)>,
}

impl<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng, E: RewardEmbedding> ChordSampler<K, N, L, R, E> for PossibleChordSampler<K, N, L, R> {
    fn new(rng: R, embedder: &E) -> Result<Self, Box<dyn std::error::Error>> {
        let chords_with_possible_probs = match get_possible_probabilities::<K, N, L, E>(embedder) {
            Ok(chords_with_probs) => chords_with_probs,
            Err(e) => return Err(e),
        };
        Ok(Self { rng, chords_with_possible_probs })
    }

    fn sample_chord(&mut self) -> Chord<K, N, L> {
        // sample chords weighted towards those that are more likely to be possible:
        // in particular, generate a random chord, and then accept it with probability equal to the estimated probability that it is possible.

        loop {
            // select a random element of possible_probs
            let (chord, possible_prob) = self.chords_with_possible_probs.choose(&mut rand::thread_rng()).unwrap();  // unwrap is safe because there are always chords
            if self.rng.gen::<f64>() < *possible_prob {
                return chord.clone()
            }
        }
    }
}

pub struct MostUncertainPossibilityChordSampler<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng> {
    rng: R,
    chords_with_possible_probs_sorted: Vec<(Chord<K, N, L>, f64)>,
}

impl<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng, E: RewardEmbedding> ChordSampler<K, N, L, R, E> for MostUncertainPossibilityChordSampler<K, N, L, R> {
    fn new(rng: R, embedder: &E) -> Result<Self, Box<dyn std::error::Error>> {
        let mut chords_with_possible_probs = match get_possible_probabilities::<K, N, L, E>(embedder) {
            Ok(chords_with_probs) => chords_with_probs,
            Err(e) => return Err(e),
        };
        chords_with_possible_probs.sort_by(|(_, p1), (_, p2)| p1.partial_cmp(p2).unwrap());
        Ok(Self { rng, chords_with_possible_probs_sorted: chords_with_possible_probs })
    }

    fn sample_chord(&mut self) -> Chord<K, N, L> {
        // sample chords weighted towards those for which the impossibility is most uncertain (i.e. closest to 1/2),
        // in particular, sample an index from a "normal distribution" over the indices, where the mean is the index of the
        // chord with the probability >= 1/2 which is closest to 1/2.
        //
        // (this isn't quite a normal distribution: if i is the index and m is the number of chords,
        // consider a binomial distribution with n = 2(m-1) and p = 1/2,
        // shifted by (n/2 - i) = m-1-i so that the mean is i and the variance is n/4 = (m-1)/2.)
        // this is our distribution except we discard any trials which yield an index < 0 or > m-1.
        let most_uncertain_idx = self.chords_with_possible_probs_sorted.iter().find_position(|(_, p)| *p >= 0.5).map(|(idx, _)| idx).unwrap_or(self.chords_with_possible_probs_sorted.len() - 1);  // i
        let binom_n = 2 * (self.chords_with_possible_probs_sorted.len() - 1);
        let binom_p = 0.5;
        // n/2 = m-1 >= i, so we can use usize instead of isize.
        let binom_shift = (self.chords_with_possible_probs_sorted.len() - 1 - most_uncertain_idx) as usize;
        let sampled_idx = loop {
            let sampled_idx_raw = (0..binom_n).map(|_| if self.rng.gen::<f64>() < binom_p { 1 } else { 0 }).sum::<usize>() as isize - (binom_shift as isize);
            if sampled_idx_raw >= 0 && sampled_idx_raw < self.chords_with_possible_probs_sorted.len() as isize {
                break sampled_idx_raw as usize;
            }
        };
        let (chord, _prob) = &self.chords_with_possible_probs_sorted[sampled_idx];
        chord.clone()
    }
}

