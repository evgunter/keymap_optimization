use gather_chord_preferences::keyboard_config_twiddler::{TwiddlerKey, TwiddlerLayout};
use strum::EnumCount;

use gather_chord_preferences::gather_chords::{gather_and_save_data, TrialResults};

const RESULTS_PATH: &str = "./data";

fn main() {
    let results_path = format!("{}/chord_preferences_results_{}.json", RESULTS_PATH, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    let gather_results = match gather_and_save_data::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>(&results_path) {
        Ok(gather_results) => gather_results,
        Err(e) => {
            eprintln!("Error gathering or saving data: {}", e);
            return;
        }
    };

    // now load the results and verify that they are the same as the originals
    let results_loaded = match TrialResults::load(&results_path) {
        Ok(results_loaded) => results_loaded,
        Err(e) => {
            eprintln!("Error loading results: {}", e);
            return;
        }
    };

    if results_loaded == gather_results {
        println!("Results loaded successfully and match the original results.");
    } else {
        eprintln!("Results loaded successfully but do not match the original results.");
        eprintln!("Original results: {:?}", gather_results);
        eprintln!("Loaded results: {:?}", results_loaded);
    }
}
