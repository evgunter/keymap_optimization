use gather_chord_preferences::keyboard_config_twiddler::{TwiddlerKey, TwiddlerLayout};
use strum::EnumCount;

use gather_chord_preferences::gather_chords::gather_data;

const RESULTS_PATH: &str = "./data";

fn main() {
    let results_path = format!("{}/chord_preferences_results_{}.json", RESULTS_PATH, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let gather_status = gather_data::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>();

    let save_status = {
        match gather_status {
            Ok(results) => results.save(&results_path),
            Err(e) => Err(e),
        }
    };

    match save_status {
        Ok(_) => println!("Results saved to {}", results_path),
        Err(e) => eprintln!("Error saving results: {}", e),
    }
}