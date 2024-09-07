use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout, TwiddlerChordTrialUtils};
use strum::EnumCount;

use keymap_optimization::chord_preferences::run;

fn main() {
    let chord_trial_utils_file = std::env::args().nth(1).expect("No chord_trial_utils_file argument provided");

    run::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, TwiddlerChordTrialUtils>(&chord_trial_utils_file);
}
