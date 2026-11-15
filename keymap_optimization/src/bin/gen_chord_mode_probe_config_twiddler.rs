// Probe tool: writes a chord-mode Twiddler config whose chords each emit one keystroke, some with modifier keys.
// Used with probe_key_events to check what the host sees for a chord: when the press and release arrive,
// whether modifiers are visible at the key press, and whether same-row and two-thumb chords register.

use keymap_optimization::twiddler::{chord_list_to_config_object, TwiddlerChord, TwiddlerKey::{self, *}};
use twidlk_rust::twiddler_config::{generate_bin_config, ChordOutput};

// usb hid modifier bits
const L_CTRL: u8 = 0x01;
const L_SHIFT: u8 = 0x02;
const L_ALT: u8 = 0x04;
const L_GUI: u8 = 0x08;
const R_CTRL: u8 = 0x10;
const R_SHIFT: u8 = 0x20;
const R_ALT: u8 = 0x40;
const R_GUI: u8 = 0x80;

const HID_KEY_A: u8 = 0x04;

// (buttons, modifiers sent with the letter). the i-th chord sends the i-th letter of the alphabet.
const PROBE_CHORDS: [(&[TwiddlerKey], u8); 22] = [
    // single buttons, no modifiers
    (&[L1], 0), (&[M1], 0), (&[R1], 0),
    (&[L2], 0), (&[M2], 0), (&[R2], 0),
    (&[L3], 0), (&[M3], 0), (&[R3], 0),
    (&[L4], 0), (&[M4], 0), (&[R4], 0),
    // several buttons in one row
    (&[L1, M1], L_CTRL),
    (&[M1, R1], L_SHIFT),
    (&[L1, M1, R1], L_ALT),
    // several rows
    (&[M1, M2], L_GUI),
    (&[M1, M2, M3], L_CTRL | L_ALT),
    (&[M1, M2, M3, M4], L_CTRL | L_SHIFT | L_ALT | L_GUI | R_CTRL),
    // thumb buttons
    (&[Z0, M1], R_CTRL),
    (&[Z0, L0, M1], R_SHIFT),
    (&[L0, M0, M1], R_ALT),
    (&[M0, R0, M1], R_GUI),
];

fn main() {
    let config_path = std::env::args().nth(1).expect("usage: gen_chord_mode_probe_config_twiddler <output .cfg path>");

    let chords_with_letters: Vec<(TwiddlerChord, String)> = PROBE_CHORDS.iter().zip('a'..='z').map(|((keys, modifiers), letter)| {
        let mut chord = TwiddlerChord::new();
        for key in keys.iter() {
            chord.add_key(*key);
        }
        println!("{} -> {} with modifier bits {:#04x}", chord, letter, modifiers);
        (chord, letter.to_string())
    }).collect();

    let mut config = chord_list_to_config_object(chords_with_letters).expect("failed to build config");
    config.key_repeat = false;
    // chord_list_to_config_object only produces plain letters and reorders the chords, so we add the modifiers
    // afterwards, finding each chord's entry in PROBE_CHORDS from the letter it sends
    for chord_with_output in config.chords.iter_mut() {
        match &mut chord_with_output.output {
            ChordOutput::SingleChord { modifier, key_code } => {
                assert_eq!(*modifier, 0, "plain letters should not come with modifiers");
                *modifier = PROBE_CHORDS[(*key_code - HID_KEY_A) as usize].1;
            }
            other => panic!("expected a single keystroke, got {:?}", other),
        }
    }

    std::fs::write(&config_path, generate_bin_config(&config).expect("failed to serialize config")).expect("failed to write config");
    println!("wrote {}", config_path);
}
