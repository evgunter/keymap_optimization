use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout, TwiddlerChordTrialUtils};
use strum::EnumCount;

use keymap_optimization::chord_preferences::data_collection_keymap_gen::run;
use keymap_optimization::local_env::DATA_PATH;
use keymap_optimization_ml::possible_chord_sampler::PossibleChordSampler;

fn main() {
    run::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, &str, PossibleChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, rand::rngs::ThreadRng>, TwiddlerChordTrialUtils>(Box::new(&DATA_PATH));
}
