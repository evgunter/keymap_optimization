use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout};
use strum::EnumCount;

use keymap_optimization::chord_preferences::run;

fn main() {
    run::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>();
}
