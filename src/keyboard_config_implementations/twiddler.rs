use crate::keyboard_config::{Chord, Layout, Key};
use rand::distributions::{Distribution, Standard};
use strum::{EnumCount, VariantArray};
use std::fmt;
use serde::{Serialize, Deserialize};

// information specific to the type of keyboard being used--in this case, a twiddler chording keyboard.

// a list of all the keys on the keyboard, with the original labels they have on the twiddler.
#[derive(Debug)]
#[derive(strum_macros::Display, strum_macros::EnumCount, strum_macros::VariantArray)]
#[derive(Serialize, Deserialize)]
#[derive(PartialEq, Clone, Copy)]
pub enum TwiddlerKey {
    Z0,  // Num
    L0,  // Alt
    M0,  // Ctrl
    R0,  // Shft
    LX,  // [left mouse button]
    MX,  // [middle mouse button]
    RX,  // [right mouse button]
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

impl Distribution<TwiddlerKey> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> TwiddlerKey {
        let index = rng.gen_range(0..TwiddlerKey::COUNT);
        TwiddlerKey::VARIANTS[index]
    }
}

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Serialize, Deserialize)]
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

impl Layout<TwiddlerKey, { TwiddlerKey::COUNT }> for TwiddlerLayout {
    fn fmt_chord(chord: &Chord<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, f: &mut fmt::Formatter) -> fmt::Result {
        let if_chord_contains = |f: &mut fmt::Formatter, key: TwiddlerKey, symb_yes: &'static str, symb_no: &'static str| -> fmt::Result {
            if chord.contains(key) {
                write!(f, "{}", symb_yes)
            } else {
                write!(f, "{}", symb_no)
            }
        };

        for key in TwiddlerLayout::THUMB {
            if_chord_contains(f, key, "⚫", "⚪")?;
        }
        writeln!(f)?;
        
        // if any of the mouse buttons are pressed, write that row; otherwise, skip the row entirely
        if TwiddlerLayout::MAIN[0].iter().any(|key| chord.contains(*key)) {
            write!(f, " ")?;  // the thumb has one more key than the rows
            for key in TwiddlerLayout::MAIN[0] {
                // uses a different color to prevent confusion
                if_chord_contains(f, key, "🔴", "⚪")?;
            }
            writeln!(f)?;
        }

        for row in TwiddlerLayout::MAIN[1..].iter() {
            write!(f, " ")?;  // the thumb has one more key than the rows
            for key in row {
                if_chord_contains(f, *key, "⚫", "⚪")?;
            }
            writeln!(f)?;
        }
        writeln!(f)
    }
}
