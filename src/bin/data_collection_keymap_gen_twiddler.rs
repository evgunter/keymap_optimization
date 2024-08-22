use keymap_optimization::twiddler::{TwiddlerKey, TwiddlerLayout, TwiddlerConfigWriterChordDecoder};
use strum::EnumCount;

use keymap_optimization::chord_preferences::data_collection_keymap_gen::run;

fn main() {
    run::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, TwiddlerConfigWriterChordDecoder>();
}