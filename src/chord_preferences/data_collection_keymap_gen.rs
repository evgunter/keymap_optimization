use rand::distributions::{Distribution, Standard};
use crate::keyboard_config::{Key, Chord, Layout, ConfigWriterChordDecoder};
use crate::local_env::RESULTS_PATH;
use std::error::Error;
use crate::chord_preferences::{random_chord, CHORD_KEY_SAMPLE_THRESHOLD};

fn gen_random_config_with_trial_decoder<K: Key, const N: usize, L: Layout<K,N>, C: ConfigWriterChordDecoder<K, N, L>>() -> Result<(String, C), Box<dyn Error>> where Standard: Distribution<K> {
    let mut rng = rand::thread_rng();

    // get a list of strings which we can legally assign to chords (i.e. not too many of them, not too long, not containing illegal characters, etc--depends on the keyboard)
    let cfg_writer_ch_decoder = C::new();
    let ok_strings = cfg_writer_ch_decoder.get_ok_strings();

    let chord_string_list: Vec<(Chord<K, N, L>, String)> = ok_strings.into_iter().map(|s| (random_chord(&mut rng, CHORD_KEY_SAMPLE_THRESHOLD), s.clone())).collect();

    // create a config file for the chord list
    let config_text = C::chords_to_config(chord_string_list)?;

    // return the config text and the decoder used to parse trial output
    Ok((config_text, cfg_writer_ch_decoder))
}

fn write_config_to_file(config: String, filename: &str) -> Result<(), std::io::Error> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(filename)?;
    file.write_all(config.as_bytes())
}

pub fn run<K: Key, const N: usize, L: Layout<K,N>, C: ConfigWriterChordDecoder<K, N, L>>() where Standard: Distribution<K> {
    let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let results_path = format!("{}/config_{}.cfg", RESULTS_PATH, current_time);

    let (config, trial_decoder) = match gen_random_config_with_trial_decoder::<K, N, L, C>() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error generating config: {}", e);
            return;
        }
    };

    match write_config_to_file(config, &results_path) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error writing config to file: {}", e);
            return;
        }
    }

    let decoder_path = format!("{}/decoder_{}.json", RESULTS_PATH, current_time);
    match write_config_to_file(serde_json::to_string(&trial_decoder).unwrap(), &decoder_path) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error writing decoder to file: {}", e);
            return;
        }
    }
}
