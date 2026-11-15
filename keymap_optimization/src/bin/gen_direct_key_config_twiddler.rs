// Probe tool: writes a Twiddler config with direct-key mode enabled and every button mapped to its own letter.
// Used with probe_key_events to find out whether the host can observe individual button presses and releases.

use keymap_optimization::twiddler::{chord_list_to_config_object, TwiddlerChord, TwiddlerKey};
use strum::{EnumCount, VariantArray};
use twidlk_rust::twiddler_config::generate_bin_config;

const OUTPUT_LETTERS: std::ops::RangeInclusive<char> = 'a'..='z';
// zip would silently drop buttons if there were more buttons than letters
const _: () = assert!(TwiddlerKey::COUNT <= 26);

fn main() {
    let config_path = std::env::args().nth(1).expect("usage: gen_direct_key_config_twiddler <output .cfg path>");

    let button_outputs: Vec<(TwiddlerChord, String)> = TwiddlerKey::VARIANTS.iter().zip(OUTPUT_LETTERS).map(|(key, letter)| {
        let mut chord = TwiddlerChord::new();
        chord.add_key(*key);
        println!("{} -> {}", key, letter);
        (chord, letter.to_string())
    }).collect();

    let mut config = chord_list_to_config_object(button_outputs).expect("failed to build config");
    config.direct_key = true;
    // autorepeat would add events that don't correspond to button movements
    config.key_repeat = false;

    std::fs::write(&config_path, generate_bin_config(&config).expect("failed to serialize config")).expect("failed to write config");
    println!("wrote {}", config_path);
}
