use gather_chord_preferences::keyboard_config::{Chord, Layout};
use gather_chord_preferences::keyboard_config_twiddler::TwiddlerLayout;
use gather_chord_preferences::keyboard_config_twiddler::TwiddlerKey::*;

fn main() {
    let k = Z0;
    println!("{}", k);
    let chord = Chord {
        keys: vec![k, L1],
    };
    let layout = TwiddlerLayout;
    layout.display_chord(chord);
    let chord2 = Chord {
        keys: vec![LX, M1],
    };
    layout.display_chord(chord2);
}
