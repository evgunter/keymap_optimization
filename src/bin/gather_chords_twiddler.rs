use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout};
use strum::EnumCount;

use keymap_optimization::chord_preferences::gather_and_save_data;

const RESULTS_PATH: &str = "./data";

fn main() {
    let results_path = format!("{}/chord_preferences_results_{}.json",
                                       RESULTS_PATH,
                                       std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    match gather_and_save_data::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>(&results_path) {
        Ok(gather_results) => gather_results,
        Err(e) => {
            eprintln!("Error gathering or saving data: {}", e);
            return;
        }
    };
}
