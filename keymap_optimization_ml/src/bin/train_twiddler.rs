use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout};
use keymap_optimization::local_env::DATA_PATH;
use strum::EnumCount;

use keymap_optimization_ml::train::run;

fn main() {
    run::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>(DATA_PATH);
}
