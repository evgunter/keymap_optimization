use rand::Rng;
use gather_chord_preferences::keyboard_config::{Key, Chord, Layout};
use rand::distributions::{Distribution, Standard};
use gather_chord_preferences::keyboard_config_twiddler::{TwiddlerKey, TwiddlerLayout};  // TODO: remove dependence on specific implementation by refactoring most of this code into a library
use strum::EnumCount;

fn sample_by_exp<R: Rng>(rng: &mut R, div: f64) -> usize {
    let normalization: f64 = (1.0 / div).exp() - 1.0;
    let quantile: f64 = rng.gen();
    let mut partial_sum: f64 = 0.0;
    let mut n: usize = 0;
    while partial_sum < quantile {
        n += 1;
        partial_sum += (-((n - 1) as f64) / div).exp() * normalization;
    }
    n
}

fn generate_random_chord_sequence<R: Rng, K: Key, const N: usize, L: Layout<K, N>>(rng: &mut R) -> Vec<Chord<K, N, L>> where Standard: Distribution<K> {
    // sample n chords according to distribution ~ e^(-(n-1)/2)
    let n_chords: usize = sample_by_exp(rng, 3.0);
    // sample m keys according to distribution ~ e^(-(m-1)/3)
    let mut chords: Vec<Chord<K, N, L>> = Vec::new();
    for _ in 0..n_chords {
        let n_keys: usize = sample_by_exp(rng, 4.0);
        let mut chord = Chord::<K, N, L>::new();
        // choose keys uniformly at random
        while chord.n_keys() < n_keys {
            let key: K = rng.gen();
            if !chord.contains(key) {
                chord.add_key(key);
            }
        }
        chords.push(chord);
    };
    chords
}

fn main() {
    let mut rng = rand::thread_rng();
    // TODO: probably this should be separated into a library with no dependence on the specific implementation of the keyboard and an executable which does depend on the implementation
    let chords: Vec<Chord<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>> = generate_random_chord_sequence(&mut rng);
    for chord in chords {
        println!("{}", chord);
    }
}