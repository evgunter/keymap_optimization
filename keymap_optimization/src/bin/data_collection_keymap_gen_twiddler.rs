use keymap_optimization::twiddler::{TwiddlerChordTrialUtils, TwiddlerKey, TwiddlerLayout};
use strum::EnumCount;

use keymap_optimization::chord_preferences::data_collection_keymap_gen::run;

fn main() {
    run::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, TwiddlerChordTrialUtils>();
}
