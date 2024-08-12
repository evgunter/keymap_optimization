use gather_chord_preferences::keyboard_config::{Chord, Layout};
use gather_chord_preferences::keyboard_config_twiddler::{TwiddlerKey, TwiddlerLayout};

fn main() {
    let k = TwiddlerKey::Z0;
    println!("{}", k);
    let chord = Chord {
        keys: vec![k, TwiddlerKey::L1],
    };
    let layout = TwiddlerLayout;
    layout.display_chord(chord);
    let chord2 = Chord {
        keys: vec![TwiddlerKey::LX, TwiddlerKey::M1],
    };
    layout.display_chord(chord2);
}
