use keymap_optimization::twiddler::{TwiddlerKey as K, TwiddlerLayout as L, TwiddlerExponentialSampler as S, TwiddlerChordTrialUtils as C};
use strum::EnumCount;
use rand::rngs::ThreadRng as R;

use keymap_optimization::chord_preferences::run;

fn main() {
    let chord_trial_utils_file = std::env::args().nth(1).expect("No chord_trial_utils_file argument provided");

    run::<K, { K::COUNT }, L, (), S<R>, C>(&chord_trial_utils_file);
}
