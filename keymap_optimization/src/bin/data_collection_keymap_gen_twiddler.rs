use keymap_optimization::twiddler::{TwiddlerKey as K, TwiddlerLayout as L, TwiddlerExponentialSampler as S, TwiddlerChordTrialUtils as C};
use strum::EnumCount;
use rand::rngs::ThreadRng as R;

use keymap_optimization::chord_preferences::data_collection_keymap_gen::run;

fn main() {
    run::<K, { K::COUNT }, L, (), S<R>, C>(&());
}
