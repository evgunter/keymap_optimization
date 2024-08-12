use crate::keyboard_config::{Chord, Layout, Key};

// Information specific to the type of keyboard being used--in this case, a Twiddler chording keyboard.

// A list of all the keys on the keyboard, with the original labels they have on the Twiddler.
#[derive(Debug)]  // TODO: remove
#[derive(strum_macros::Display)]
#[derive(PartialEq)]
#[derive(Clone)]
#[derive(Copy)]
pub enum TwiddlerKey {
    Z0,  // Num
    L0,  // Alt
    M0,  // Ctrl
    R0,  // Shft
    LX,  // [Left mouse button]
    MX,  // [Middle mouse button]
    RX,  // [Right mouse button]
    L1,  // A
    M1,  // E
    R1,  // SP
    L2,  // B
    M2,  // F
    R2,  // DEL
    L3,  // C
    M3,  // G
    R3,  // BS
    L4,  // D   
    M4,  // H
    R4,  // ENT
}

impl Key for TwiddlerKey {}

pub struct TwiddlerLayout;

impl TwiddlerLayout {
    const THUMB: [TwiddlerKey; 4] = [
        TwiddlerKey::Z0,
        TwiddlerKey::L0,
        TwiddlerKey::M0,
        TwiddlerKey::R0,
    ];

    const MAIN: [[TwiddlerKey; 3]; 5] = [
        [TwiddlerKey::LX, TwiddlerKey::MX, TwiddlerKey::RX],
        [TwiddlerKey::L1, TwiddlerKey::M1, TwiddlerKey::R1],
        [TwiddlerKey::L2, TwiddlerKey::M2, TwiddlerKey::R2],
        [TwiddlerKey::L3, TwiddlerKey::M3, TwiddlerKey::R3],
        [TwiddlerKey::L4, TwiddlerKey::M4, TwiddlerKey::R4],
    ];
}

impl Layout<TwiddlerKey> for TwiddlerLayout {
    fn display_chord(&self, chord: Chord<TwiddlerKey>) {
        let if_chord_contains = |key: TwiddlerKey, symb_yes: &'static str, symb_no: &'static str| -> () {
            if chord.keys.contains(&key) {
                print!("{}", symb_yes);
            } else {
                print!("{}", symb_no);
            }
        };

        for key in TwiddlerLayout::THUMB {
            if_chord_contains(key, "⚫", "⚪");
        }
        println!();
        
        // if any of the mouse buttons are pressed, print that row; otherwise, skip the row entirely
        if TwiddlerLayout::MAIN[0].iter().any(|key| chord.keys.contains(key)) {
            print!(" ");  // The thumb has one more key than the rows
            for key in TwiddlerLayout::MAIN[0] {
                // Uses a different color to prevent confusion
                if_chord_contains(key, "🔴", "⚪");
            }
            println!();
        }

        for row in TwiddlerLayout::MAIN {
            print!(" ");  // The thumb has one more key than the rows
            for key in row {
                if_chord_contains(key, "⚫", "⚪");
            }
            println!();
        }
        println!();
    }
}

