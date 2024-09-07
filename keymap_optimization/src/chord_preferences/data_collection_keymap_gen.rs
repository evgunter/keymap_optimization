use rand::distributions::{Distribution, Standard};
use crate::keyboard_config::{Key, Layout, ChordTrialUtils};
use crate::local_env::RESULTS_PATH;
use std::error::Error;

pub fn gen_random_config_with_trial_decoder<K: Key, const N: usize, L: Layout<K,N>, C: ChordTrialUtils<K, N, L>>() -> Result<(Vec<u8>, C), Box<dyn Error>> where Standard: Distribution<K> {
    // create a legal vocabulary of chords, and a decoder for the trial output.
    // return the text of a keyboard config file and the decoder used to parse trial output
    let chord_trial_utils = C::new();
    Ok((chord_trial_utils.get_config()?, chord_trial_utils))
}

pub fn run<K: Key, const N: usize, L: Layout<K,N>, C: ChordTrialUtils<K, N, L>>() where Standard: Distribution<K> {
    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let results_path = format!("{}/config_{}.cfg", RESULTS_PATH, current_time);

    let (config, trial_decoder) = match gen_random_config_with_trial_decoder::<K, N, L, C>() {
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

    let decoder_path = format!("{}/decoder_{}.json", RESULTS_PATH, current_time);
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
