
use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout};
use strum::EnumCount;

use keymap_optimization_ml::reward_model::run;

fn main() {
    run::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>();
}
