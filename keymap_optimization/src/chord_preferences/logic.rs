use rand::Rng;
use crate::keyboard_config::{Key, Chord, Layout};
use rand::distributions::{Distribution, Standard};
use serde::{Serialize, Deserialize, de::DeserializeOwned};

const N_REPETITIONS_PER_TRIAL: usize = 5;

#[derive(PartialEq, Debug)]
#[derive(Serialize, Deserialize)]
pub enum ErrCode {
    Impossible,
}

#[derive(PartialEq, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(bound = "K: DeserializeOwned, L: DeserializeOwned")]
pub struct TrialData<K: Key, const N: usize, L: Layout<K, N>> where Standard: Distribution<K> {
    pub chord_pair: [Chord<K, N, L>; 2],
    pub n_repetitions: usize,
    pub time: Result<f64, ErrCode>,
}

#[derive(PartialEq, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(bound = "K: DeserializeOwned, L: DeserializeOwned")]
pub struct TrialResults<K: Key, const N: usize, L: Layout<K, N>> where Standard: Distribution<K> {
    pub data: Vec<TrialData<K, N, L>>,
}

impl<K: Key, const N: usize, L: Layout<K, N>> TrialResults<K, N, L> where Standard: Distribution<K> {
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

    pub fn load(filename: &str) -> std::io::Result<Self> {
        let file = std::fs::File::open(filename)?;
        let results = serde_json::from_reader(file)?;
        Ok(results)
    }
}

pub fn random_chord<R: Rng, K: Key, const N: usize, L: Layout<K, N>>(rng: &mut R, threshold: f64) -> Chord<K, N, L> where Standard: Distribution<K> {
    // Sample a random chord with a number of keys distributed almost exponentially with base 1/threshold
    // (not exactly exponential because we are sampling with replacement and we always sample at least one key)
    let mut chord = Chord::new();
    chord.add_key(rng.gen::<K>());  // ensure that the chord contains at least one key
    loop {
        let val: f64 = rng.gen::<f64>();
        if val < threshold {
            chord.add_key(rng.gen::<K>());
        } else {
            break;
        }
    }
    chord
}

fn get_expected_input<K: Key, const N: usize, L: Layout<K, N>>(chords: &[Chord<K, N, L>; 2]) -> String where Standard: Distribution<K> {
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

fn gather_data<K: Key, const N: usize, L: Layout<K, N>>() -> Result<TrialResults<K, N, L>, std::io::Error> where Standard: Distribution<K> {
    let mut rng = rand::thread_rng();
    println!("You will be shown two chords. After some time to practice, you will need to type this pair of chords {} times, as quickly as possible.", N_REPETITIONS_PER_TRIAL);
    
    let mut results: TrialResults<K, N, L> = TrialResults::new();
    const THRESHOLD: f64 = 0.8;

    // Run trials until the user quits
    loop {
        let chords = [random_chord(&mut rng, THRESHOLD), random_chord(&mut rng, THRESHOLD)];
        for chord in &chords {
            println!("{}", chord);
        }

        'trial: loop {
            let mut practice_input = String::new();
            println!("Type GO when you're ready to continue, IMP if this contains an impossible combination, SKIP to skip this pair without recording any data, or QUIT to quit. Hit Enter after you're done typing the chords.");
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
                            time: Ok(trial_time),
                        };
                        results.push(trial_data);
                        break 'trial;
                    } else if accept_input == "N\n" {
                        break 'accept;
                    } else {
                        println!("Please type Y or N.");
                    }
                }
            } else if practice_input == "SKIP\n" {
                break 'trial;
            } else if practice_input == "IMP\n" {
                let trial_data = TrialData {
                    chord_pair: chords,
                    n_repetitions: N_REPETITIONS_PER_TRIAL,
                    time: Err(ErrCode::Impossible),
                };
                results.push(trial_data);
                break 'trial;
            } else if practice_input == "QUIT\n" {
                println!("Quitting...");
                return Ok(results);
            }
        }


    }
}

pub fn gather_and_save_data<K: Key, const N: usize, L: Layout<K, N>>(filename: &str) -> Result<TrialResults<K, N, L>, std::io::Error> where Standard: Distribution<K> {
    let results = gather_data::<K, N, L>()?;
    results.save(filename)?;
    Ok(results)
}
