use rand::Rng;
use crate::keyboard_config::{Key, Chord, Layout};
use rand::distributions::{Distribution, Standard};
use serde::{Serialize};

const N_REPETITIONS_PER_TRIAL: usize = 5;

#[derive(Serialize)]
pub struct TrialData<K: Key, const N: usize, L: Layout<K, N>> {
    chord_pair: [Chord<K, N, L>; 2],
    n_repetitions: usize,
    time: f64,
}

#[derive(Serialize)]
pub struct TrialResults<K: Key, const N: usize, L: Layout<K, N>> {
    pub data: Vec<TrialData<K, N, L>>,
}

impl<K: Key, const N: usize, L: Layout<K, N>> TrialResults<K, N, L> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
        }
    }

    pub fn push(&mut self, trial_data: TrialData<K, N, L>) {
        self.data.push(trial_data);
    }

    pub fn save(&self, filename: &str) -> std::io::Result<()> {
        let file = std::fs::File::create(filename)?;
        serde_json::to_writer(file, self)?;
        Ok(())
    }
}

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

fn generate_random_chord_pair<R: Rng, K: Key, const N: usize, L: Layout<K, N>>(rng: &mut R) -> [Chord<K, N, L>; 2] where Standard: Distribution<K> {
    // sample m keys according to distribution ~ e^(-(m-1)/3)
    let mut chords: [Chord<K, N, L>; 2] = [Chord::new(), Chord::new()];
    for chord in chords.iter_mut() {
        let n_keys: usize = sample_by_exp(rng, 4.0);
        // choose keys uniformly at random
        while chord.n_keys() < n_keys {
            let key: K = rng.gen();
            if !chord.contains(key) {
                chord.add_key(key);
            }
        }
    };
    chords
}

fn get_expected_input<K: Key, const N: usize, L: Layout<K, N>>(chords: &[Chord<K, N, L>; 2]) -> String {
    let mut expected = String::new();
    for chord in chords {
        for key in K::VARIANTS.iter() {
            if chord.contains(*key) {
                expected.push_str(&format!("{}", key));
            }
        }
    }
    expected
}

fn count_errors(_actual_input: &str, _expected_input: String) -> usize {
    // TODO: implement
    0
}

pub fn gather_data<K: Key, const N: usize, L: Layout<K, N>>() -> Result<TrialResults<K, N, L>, std::io::Error> where Standard: Distribution<K> {
    let mut rng = rand::thread_rng();
    println!("You will be shown two chords. After some time to practice, you will need to type this pair of chords {} times, as quickly as possible.", N_REPETITIONS_PER_TRIAL);
    
    let mut results: TrialResults<K, N, L> = TrialResults::new();

    // Run trials until the user quits
    loop {
        let chords: [Chord<K, N, L>; 2] = generate_random_chord_pair(&mut rng);
        for chord in &chords {
            println!("{}", chord);
        }

        'trial: loop {
            let mut practice_input = String::new();
            println!("Type GO when you're ready to continue, or QUIT to quit. Hit Enter after you're done typing the chords.");
            std::io::stdin().read_line(&mut practice_input)?;
            if practice_input == "GO\n" {
                let mut trial_input = String::new();
                let start_time = std::time::Instant::now();
                std::io::stdin().read_line(&mut trial_input)?;
                let trial_time = start_time.elapsed().as_secs_f64();
                println!("Average time: {}; accuracy: {}", trial_time / (2 * N_REPETITIONS_PER_TRIAL - 1) as f64, count_errors(&trial_input, get_expected_input(&chords)));
                println!("Expected: {}", get_expected_input(&chords));
                println!("Accept this trial (Y), or try again (N)?");
                'accept: loop {
                    let mut accept_input = String::new();
                    std::io::stdin().read_line(&mut accept_input)?;
                    println!("");
                    if accept_input == "Y\n" {
                        let trial_data = TrialData {
                            chord_pair: chords,
                            n_repetitions: N_REPETITIONS_PER_TRIAL,
                            time: trial_time,
                        };
                        results.push(trial_data);
                        break 'trial;
                    } else if accept_input == "N\n" {
                        break 'accept;
                    } else {
                        println!("Please type Y or N.");
                    }
                }
            } else if practice_input == "QUIT\n" {
                println!("Quitting...");
                return Ok(results);
            }
        }


    }
}
