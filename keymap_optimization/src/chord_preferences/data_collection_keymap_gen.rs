use crate::keyboard_config::{Key, Layout, ChordTrialUtils, ChordSampler};
use crate::local_env::DATA_PATH;
use std::error::Error;

pub fn gen_random_config_with_trial_decoder<K: Key, const N: usize, L: Layout<K,N>, I, S: ChordSampler<K, N, L, rand::rngs::ThreadRng, I>, C: ChordTrialUtils<K, N, L, rand::rngs::ThreadRng, I, S>>(initialization_info: Box<I>) -> Result<(Vec<u8>, C), Box<dyn Error>> {
    // create a legal vocabulary of chords, and a decoder for the trial output.
    // return the text of a keyboard config file and the decoder used to parse trial output
    let chord_trial_utils = C::new(S::new(rand::thread_rng(), initialization_info)?);
    Ok((chord_trial_utils.get_config()?, chord_trial_utils))
}

pub fn run<'a, K: Key, const N: usize, L: Layout<K,N>, I, S: ChordSampler<K, N, L, rand::rngs::ThreadRng, I>, C: ChordTrialUtils<K, N, L, rand::rngs::ThreadRng, I, S>>(initialization_info: Box<I>) {
    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let results_path = format!("{}/config_{}.cfg", DATA_PATH, current_time);

    let (config, trial_decoder) = match gen_random_config_with_trial_decoder::<K, N, L, I, S, C>(initialization_info) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error generating config: {}", e);
            return;
        }
    };

    match std::fs::write(&results_path, config) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("error writing config to file: {}", e);
            return;
        }
    }

    let decoder_path = format!("{}/decoder_{}.json", DATA_PATH, current_time);
    match std::fs::write(&decoder_path, serde_json::to_string(&trial_decoder).unwrap()) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("error writing decoder to file: {}", e);
            return;
        }
    }

    println!("generated config file:\n{}", results_path);
    println!("generated decoder file:\n{}", decoder_path);
}
