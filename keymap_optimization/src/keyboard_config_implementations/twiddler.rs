use crate::keyboard_config::{Chord, ChordTrialUtils, Key, Layout, ChordSampler};
use rand::distributions::{Distribution, Standard};
use rand::rngs::ThreadRng;
use strum::{EnumCount, VariantArray};
use std::fmt;
use std::fmt::Display;
use std::error::Error;
use serde::{Serialize, Deserialize};
use serde_big_array::BigArray;
use queues::{queue, Queue, IsQueue};

use twidlk_rust::{twiddler_config::{generate_bin_config, text_to_usb, usb_hid_to_text, sort_chords, ChordWithOutput, TwiddlerConfig}, unmap_char};

// requirements for twiddler config files
const MAX_CHORDS: u16 = 1020;
const MAX_MULTICHAR_CHORDS: u16 = 256;

// we use usb hid codes to represent characters in the output since they're what the twiddler actually sends;
// we aren't working with the codes directly (we're basically just using the number of them) but it's nice
// to have them tied to the actual table.
type Idx = u8;
type Usb = u8;  // (shifted, code)

const USB_HID_RANGES: [(Usb, Usb); 3] = [
    (0x04, 0x28),  // alphanumeric + numbers
    (0x2d, 0x32),  // some special characters
    (0x33, 0x39)   // more special characters (we skip non-US # and ~)
    // skip whitespace, escape, backspace
];

macro_rules! public_for_test {
    ($(#[$meta:meta])* $vis:vis const $name:ident: $type:ty = $body:expr;) => {
        #[cfg(test)]
        $(#[$meta])*
        pub(crate) const $name: $type = $body;

        #[cfg(not(test))]
        $(#[$meta])*
        $vis const $name: $type = $body;
    };

    ($(#[$meta:meta])* $vis:vis struct $name:ident $body:tt) => {
        #[cfg(test)]
        $(#[$meta])*
        pub(crate) struct $name $body

        #[cfg(not(test))]
        $(#[$meta])*
        $vis struct $name $body
    };

    ($(#[$meta:meta])* $vis:vis fn $name:ident$(<$($($gen_arg:ident)*: $gen_trait:path),*>)?($($arg:ident: $typ:ty),*) $(-> $ret:ty)? $(where $($b:path: $d:path),*)? $body:block) => {
        #[cfg(test)]
        $(#[$meta])*
        pub(crate) fn $name$(<$($($gen_arg)*: $gen_trait),*>)?($($arg: $typ),*) $(-> $ret)? $(where $($b: $d),*)? $body

        #[cfg(not(test))]
        $(#[$meta])*
        $vis fn $name$(<$($($gen_arg)*: $gen_trait),*>)?($($arg: $typ),*) $(-> $ret)? $(where $($b: $d),*)? $body
    };

    ($(#[$meta:meta])* $vis:vis type $name:ident = $type:ty;) => {
        #[cfg(test)]
        $(#[$meta])*
        pub(crate) type $name = $type;

        #[cfg(not(test))]
        $(#[$meta])*
        $vis type $name = $type;
    };

}

// the overall count is thisx2 because shifted differs from unshifted
const HALF_USB_HID_COUNT: u8 = USB_HID_RANGES[0].1 - USB_HID_RANGES[0].0
                             + USB_HID_RANGES[1].1 - USB_HID_RANGES[1].0
                             + USB_HID_RANGES[2].1 - USB_HID_RANGES[2].0;

public_for_test! {
const USB_HID_COUNT: u8 = 2 * HALF_USB_HID_COUNT;
}
                         

// information specific to the type of keyboard being used--in this case, a twiddler chording keyboard.

// === types for representing the twiddler keyboard ===

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
    // LX,  // [left mouse button]  // these can't be used in chords, so i think it's not useful to include them
    // MX,  // [middle mouse button]
    // RX,  // [right mouse button]
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

impl Key for TwiddlerKey {
    fn gen_random<R: rand::Rng>(rng: &mut R) -> Self {
        rng.gen::<TwiddlerKey>()
    }
}

impl Distribution<TwiddlerKey> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> TwiddlerKey {
        let index = rng.gen_range(0..TwiddlerKey::COUNT);
        TwiddlerKey::VARIANTS[index]
    }
}

#[derive(Debug)]
#[derive(PartialEq, Clone)]
#[derive(Serialize, Deserialize)]
pub struct TwiddlerLayout;

impl TwiddlerLayout {
    public_for_test! {
    const THUMB: [TwiddlerKey; 4] = [
        TwiddlerKey::Z0,
        TwiddlerKey::L0,
        TwiddlerKey::M0,
        TwiddlerKey::R0,
    ];
    }

    // these can't be used in chords, so i think it's not useful to include them
    // const MOUSE: [TwiddlerKey; 3] = [
        // TwiddlerKey::LX,
        // TwiddlerKey::MX,
        // TwiddlerKey::RX,
    // ];

    public_for_test! {
    const MAIN: [[TwiddlerKey; 3]; 4] = [
        // [TwiddlerKey::LX, TwiddlerKey::MX, TwiddlerKey::RX],  // these can't be used in chords, so i think it's not useful to include them
        [TwiddlerKey::L1, TwiddlerKey::M1, TwiddlerKey::R1],
        [TwiddlerKey::L2, TwiddlerKey::M2, TwiddlerKey::R2],
        [TwiddlerKey::L3, TwiddlerKey::M3, TwiddlerKey::R3],
        [TwiddlerKey::L4, TwiddlerKey::M4, TwiddlerKey::R4],
    ];
    }
}

pub type TwiddlerChord = Chord<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>;

impl Layout<TwiddlerKey, { TwiddlerKey::COUNT }> for TwiddlerLayout {
    fn fmt_chord_graphical(chord: &TwiddlerChord, f: &mut fmt::Formatter) -> fmt::Result {
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

        for row in TwiddlerLayout::MAIN.iter() {
            write!(f, " ")?;  // the thumb has one more key than the rows
            for key in row {
                if_chord_contains(f, *key, "⚫", "⚪")?;
            }
            writeln!(f)?;
        }
        writeln!(f)
    }
    fn fmt_chord_text(chord: &TwiddlerChord, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        chord_my_format_to_twidlk(chord.clone()).fmt(f)
    }

    fn is_valid(chord: &TwiddlerChord) -> bool {
        // a chord is valid if it contains at least one non-thumb key and is not a reserved chord
        // (for at least some of the "reserved" chords, you actually can overwrite it and it works.
        // but they're not terribly useful chords anyway (all requiring both num and shift) so i'll just skip them)

        if !TwiddlerLayout::MAIN.concat().into_iter().any(|k| chord.contains(k)) {
            false
        } else {
            for reserved_chord in RESERVED {
                let mut reserved_chord_tw = TwiddlerChord::new();
                for key in reserved_chord {
                    reserved_chord_tw.add_key(key);
                }
                if reserved_chord_tw == *chord {
                    return false;
                }
            }
            true
        }
    }
}

// Z0, R0 is also reserved but isn't a valid chord anyway
pub const RESERVED: [[TwiddlerKey; 3]; 8] = [
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::R1],
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::R2],
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::R3],
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::R4],
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::M1],
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::M2],
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::M3],
    [TwiddlerKey::Z0, TwiddlerKey::R0, TwiddlerKey::M4],
];

// === utilities for writing twiddler config files ===

fn empty_config() -> TwiddlerConfig {
    TwiddlerConfig {
        version: (),
        key_repeat: true,
        direct_key: false,
        joystick_left_click: false,
        disable_bluetooth: false,
        sticky_num: false,
        sticky_shift: false,
        haptic_feedback: false,
    
        sleep_timeout: 300,
        mouse_left_click_action: 0,
        mouse_middle_click_action: 0,
        mouse_right_click_action: 0,
        mouse_accel_factor: 255,
        key_repeat_delay: 100,
    
        chords: Vec::new(),
    }    
}

fn chord_my_format_to_twidlk(my_format_chord: TwiddlerChord) -> twidlk_rust::Chord {
    let twidlk_key_to_my_format_key: Vec<(TwiddlerKey, u16)> = vec![
        (TwiddlerKey::Z0, 0),
        (TwiddlerKey::L0, 4),
        (TwiddlerKey::M0, 8),
        (TwiddlerKey::R0, 12),
        (TwiddlerKey::L1, 1),
        (TwiddlerKey::M1, 2),
        (TwiddlerKey::R1, 3),
        (TwiddlerKey::L2, 5),
        (TwiddlerKey::M2, 6),
        (TwiddlerKey::R2, 7),
        (TwiddlerKey::L3, 9),
        (TwiddlerKey::M3, 10),
        (TwiddlerKey::R3, 11),
        (TwiddlerKey::L4, 13),
        (TwiddlerKey::M4, 14),
        (TwiddlerKey::R4, 15),
    ];

    let twidlk_chord = twidlk_rust::Chord {
        keys: twidlk_key_to_my_format_key.iter()
              .filter(|(my_key, _)| my_format_chord.contains(*my_key))
              .map(|(_, twidlk_key)| *twidlk_key)
              .collect()
    };
    twidlk_chord
}

#[derive(Serialize, Deserialize)]
struct Children {
    #[serde(with = "BigArray")]
    contents: [Node; USB_HID_COUNT as usize],
}

public_for_test!{
#[derive(Serialize, Deserialize)]
struct Node {
    // the value is implicit in its index in its parent's children array
    children: Option<Box<Children>>,
}
}

impl Node {
    // these are only actually public for tests, but Node itself is private so that's ok
    pub fn idx_to_usb(idx: Idx) -> Result<(bool, Usb), Box<dyn Error>> {
        let (shifted, base_idx) = (idx/HALF_USB_HID_COUNT != 0, idx % HALF_USB_HID_COUNT);

        Ok((shifted, if base_idx < USB_HID_RANGES[0].1 - USB_HID_RANGES[0].0 {
            base_idx + USB_HID_RANGES[0].0
            } else if base_idx < USB_HID_RANGES[0].1 - USB_HID_RANGES[0].0 + USB_HID_RANGES[1].1 - USB_HID_RANGES[1].0 {
                base_idx + USB_HID_RANGES[0].0 + USB_HID_RANGES[1].0 - USB_HID_RANGES[0].1
            } else {
                base_idx + USB_HID_RANGES[0].0 + USB_HID_RANGES[1].0 - USB_HID_RANGES[0].1 + USB_HID_RANGES[2].0 - USB_HID_RANGES[1].1
            }
        ))
    }

    // these are only actually public for tests, but Node itself is private so that's ok
    pub fn usb_to_idx(shifted: bool, usb: Usb) -> Result<Idx, Box<dyn Error>> {
        let base_decoded = if usb >= USB_HID_RANGES[0].0 && usb < USB_HID_RANGES[0].1 {
            usb - USB_HID_RANGES[0].0
        } else if usb >= USB_HID_RANGES[1].0 && usb < USB_HID_RANGES[1].1 {
            usb - (USB_HID_RANGES[1].0 - USB_HID_RANGES[0].1) - USB_HID_RANGES[0].0
        } else if usb >= USB_HID_RANGES[2].0 && usb < USB_HID_RANGES[2].1 {
            usb - (USB_HID_RANGES[2].0 - USB_HID_RANGES[1].1) - (USB_HID_RANGES[1].0 - USB_HID_RANGES[0].1) - USB_HID_RANGES[0].0

        } else {
            return Err(format!("usb code out of range: {}", usb).into())
        };
        // put all the indices for shifted codes after the unshifted and agnostic ones
        if shifted {
            Ok(base_decoded + HALF_USB_HID_COUNT)
        } else {
            Ok(base_decoded)
        }
    }

    fn idx_to_string(idx: Idx) -> Result<String, Box<dyn Error>> {
        let (shifted, usb) = Node::idx_to_usb(idx)?;
        Ok(usb_hid_to_text(shifted, usb).1)
    }

    fn idxs_to_string(idxs: Vec<Idx>) -> Result<String, Box<dyn Error>> {
        // convert a list of indices to a single string by concatenating the results for each index
        idxs.into_iter().map(|i| Node::idx_to_string(i)).collect()
    }

    // value is reversed. it should be cloned before calling this function.
    fn get_child<'a>(&mut self, mut value: Vec<Idx>) -> Option<&mut Node> {
        // just get_child_ but with value reversed
        value.reverse();
        self.get_child_(value)
    }

    fn get_child_(&mut self, mut value: Vec<Idx>) -> Option<&mut Node> {
        match value.pop() {
            None => return Some(self),
            Some(last) => {
                match &mut self.children {
                    Some(children) => children.contents[last as usize].get_child_(value),
                    None => None,
                }
            }
        }
    }

    fn read_last_word_<'a>(&self, out: &'a mut Vec<Idx>, value: &'a mut Vec<Idx>) -> Result<(), Box<dyn Error>> {
        // removes the last word of value and reads it into out
        match value.pop() {
            None => match &self.children {
                None => Ok(()),  // the value string ended at a leaf node
                Some(_) => Err("value string ended in the middle of a word".into()),
            }
            Some(last) => {
                match &self.children {
                    None => {
                        value.push(last);  // put last back, since it wasn't part of this word
                        Ok(())
                    },
                    Some(children) => {
                        out.push(last);
                        children.contents[last as usize].read_last_word_(out, value)
                    }
                }
            }
        }
    }

    fn read_last_word<'a>(&self, value: &'a mut Vec<Idx>) -> Result<Vec<Idx>, Box<dyn Error>> {
        // removes the last word of value and returns it as a string
        let mut out = Vec::new();
        self.read_last_word_(&mut out, value)?;
        Ok(out)
    }
}

#[derive(Serialize, Deserialize)]
pub struct TwiddlerChordTrialUtils {
    vocab: Vec<(TwiddlerChord, String)>,
    code_tree: Node,
}

impl TwiddlerChordTrialUtils {
    // this should only be called once: during initialization. after that, the fields vocab and code_tree should be referenced.
    fn get_code<R: rand::Rng, I, S: ChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, R, I>>(chord_sampler: &mut S) -> (Node, Vec<(TwiddlerChord, String)>) {
        // make a binary tree so we can uniquely decode sequences of chord strings into chords
        // there can be at most MAX_MULTICHAR_CHORDS strings with multiple characters,
        // and at most MAX_CHORDS strings overall.

        let mut multichar_count: u16 = 0;

        // the string represents the path from the root (empty string) to the leaf
        let root_value = Vec::new();
        let mut root = Node {
            children: None,
        };
        // the queue just stores the in-progress strings, rather than mutable references to them, to avoid multiple mutable borrows.
        // when we want to modify the children of a node, we look it up via the tree.
        let mut node_queue: Queue<Vec<Idx>> = queue![];
        node_queue.add(root_value).unwrap();  // for whatever reason this always returns Ok(None). idk why. i checked the source code. so the unwrap is ok
        'create_strings: loop {
            // take a node from the queue.
            // create and enqueue all its children.
            // continue this until we reach one of the stopping conditions.
            // this unwrap is safe because we always enqueue at least one child for each node
            // (in particular, we always enqueue as many children as there are HID codes we want to use)
            let current_node_str = node_queue.remove().unwrap();
            // this unwrap is safe because whenever we add a string to the queue we also add it to the tree, and we never remove things from the tree
            let current_node = if current_node_str.len() == 0 {  // this only happens when we just popped the root
                &mut root
            } else {
                root.get_child(current_node_str.clone()).unwrap()
            };
            if current_node_str.len() > 1 {  // we're removing this string from the queue, so if it has multiple characters we need to adjust the count
                multichar_count -= 1;
            }

            // use core::array::from_fn to create an array of nodes
            current_node.children = Some(Box::new(Children { contents: core::array::from_fn(|_| Node { children: None }) }));

            for idx in 0..USB_HID_COUNT {
                let mut new_node_str = current_node_str.clone();
                new_node_str.push(idx);
                node_queue.add(new_node_str).unwrap();  // for whatever reason this always returns Ok(None). idk why. i checked the source code. so the unwrap is ok
                if current_node_str.len() > 0 {  // if the current node contained at least one character, the new value we're adding is a multichar
                    multichar_count += 1;
                }
                // stopping conditions
                if node_queue.size() >= MAX_CHORDS as usize || multichar_count >= MAX_MULTICHAR_CHORDS {
                    break 'create_strings;
                }
            }
        }
        fn queue_to_vec<T: Clone>(mut queue: Queue<T>) -> Vec<T> {
            let mut vec = Vec::new();
            while let Ok(item) = queue.remove() {
                vec.push(item);
            }
            vec
        }

        // now the queue contains a valid set of strings
        let ok_strings = queue_to_vec(node_queue)
        .into_iter()
        // this unwrap is safe if the code is correct, because the values of i that are converted to usb do not depend on any input
        .map(|s| Node::idxs_to_string(s).unwrap())
        .collect::<Vec<String>>();

        // we match each string with a chord
        let mut chords = Vec::new();
        while chords.len() < ok_strings.len() {
            let chord = chord_sampler.sample_chord();
            if !chords.contains(&chord) {
                chords.push(chord);
            }
        }
        let vocab = chords.into_iter().zip(ok_strings).collect();

        (root, vocab)
    }

}

pub fn chord_list_to_config_object(chords: Vec<(TwiddlerChord, String)>) -> Result<TwiddlerConfig, Box<dyn Error>> {
    // takes a list of (chord, output_string) pairs, and creates a TwiddlerConfig with the default settings and the input chords
    let mut twidlk_config = empty_config();
    for (chord, output_str) in chords {
        let twidlk_chord = chord_my_format_to_twidlk(chord);
        let twidlk_chord_output = text_to_usb(output_str)?;
        twidlk_config.chords.push(ChordWithOutput { chord: twidlk_chord, output: twidlk_chord_output });
    }
    sort_chords(&mut twidlk_config.chords);
    Ok(twidlk_config)
}

public_for_test! {
fn random_chord_<R: rand::Rng, K: Key, const N: usize, L: Layout<K, N>>(rng: &mut R, threshold: f64) -> Chord<K, N, L> {
    // sample a random chord with a number of keys distributed almost exponentially with base 1/threshold
    // (not exactly exponential because we are sampling with replacement and we always sample at least one key)
    let mut chord = Chord::new();
    chord.add_key(K::gen_random(rng));  // ensure that the chord contains at least one key
    loop {
        let val: f64 = rng.gen::<f64>();
        if val < threshold {
            chord.add_key(K::gen_random(rng));
        } else {
            break;
        }
    }
    chord
}
}

pub struct TwiddlerExponentialSampler<R: rand::Rng> {
    rng: R
}

impl ChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, ThreadRng, ()> for TwiddlerExponentialSampler<ThreadRng> {
    fn new(rng: ThreadRng, _: Box<()>) -> Result<Self, Box<dyn Error>> {
        Ok(TwiddlerExponentialSampler { rng })
    }

    fn sample_chord(&mut self) -> TwiddlerChord {
        // sample a chord with an exponentially distributed number of keys
        const CHORD_KEY_SAMPLE_THRESHOLD: f64 = 0.6;
        // rejection sample until we get a valid chord (this is quite fast; most chords are valid)
        loop {
            let attempted_chord = random_chord_(&mut self.rng, CHORD_KEY_SAMPLE_THRESHOLD);
            if TwiddlerLayout::is_valid(&attempted_chord) {
                return attempted_chord;
            }
        }
    }
}

impl<I, S: ChordSampler<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, ThreadRng, I>> ChordTrialUtils<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, ThreadRng, I, S> for TwiddlerChordTrialUtils {
    fn new(mut chord_sampler: S) -> Self {
        let (code_tree, vocab) = Self::get_code(&mut chord_sampler);
        TwiddlerChordTrialUtils {
            vocab,
            code_tree,
        }
    }

    fn get_vocab(&self) -> &Vec<(TwiddlerChord, String)> {
        &self.vocab
    }

    fn get_config(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let twidlk_config = chord_list_to_config_object(self.vocab.clone())?;
        generate_bin_config(&twidlk_config)
    }

    fn parse_trial_string(&self, trial_string: &str) -> Result<Vec<TwiddlerChord>, Box<dyn Error>> {
        // convert the test string to usb hid codes, and from there to indices
        let mut trial_idxs = trial_string.chars().map(|c| {
            let (shifted, usb) = unmap_char(&c.to_string())?;
            Node::usb_to_idx(match shifted {
                Some(v) => v,
                _ => false,
            }, usb)
        }).collect::<Result<Vec<Idx>, Box<dyn Error>>>()?;
        let root = &self.code_tree;

        // the reader function takes a reversed list
        trial_idxs.reverse();

        // read in words until end of trial input
        let mut words: Vec<String> = Vec::new();
        while trial_idxs.len() > 0 {
            let word: String = root.read_last_word(&mut trial_idxs)?
            .into_iter()
            .map(|i| Node::idx_to_usb(i).and_then(|(s, c)| Ok(usb_hid_to_text(s, c).1)))
            .collect::<Result<Vec<String>, Box<dyn Error>>>()?
            .join("");
            words.push(word);
        }

        // now convert the words to chords
        let result: Vec<TwiddlerChord> = match words.into_iter().map(|w| <TwiddlerChordTrialUtils as ChordTrialUtils<TwiddlerKey, 16, TwiddlerLayout, ThreadRng, I, S>>::lookup_string(self, &w)).collect() {
            None => return Err("could not find chord for word".into()),
            Some(c) => c,
        };

        Ok(result)
    }
}

// is it possible to do some crazy overlapping thing with the strings table so i can have more of them?
// e.g. by having them all be at most 2 characters, or by having them overlap or something?
// --> tl;dr no.
//  unfortunately the strings block is written as alternating lengths and string contents, so this doesn't work nearly as well as it
// would if it didn't contain the lengths, only the contents--the lengths are u16 rather than u8 for some reason, and they're little-endian,
// so the second byte is usually zero; the characters are also two bytes, the first containing modifiers
// (so, 00 unless shift/num lock/ctrl/alt/gui is pressed) followed by the ascii code, so usually their first byte is zero.
// so, to read part of a string contents as a length (required to add additional codes by pointing the start to a place not originally
// designated as a length), you'd need to either have an output > 255 characters long (and, since the rest of that is going to be
// whatever misaligned thing comes after that, there's no way that's useful; it'll almost certainly contain invalid characters (such as null)
// when reading other shifted length codes), or you need to read most of the characters with the modifier swapped with the ascii,
// which only works at all if the modifier was not zero (ascii code 0 is null), and means that the output will have at least one
// but probably a bunch of modifiers applied;
// in fact, i'm not sure if it works at all since it was hard to find an instance where this also didn't include any invliad characters
// so, this is also nearly useless. (also, i'm not sure if it works in practice--they certainly don't work when they contain null, but
// also i tried some others, including one that should've printed Ctrl-Alt-Gui-e, but that didn't work either. however, it may still be possible;
// my example used an odd-indexed pointer, which might be a problem.) unfortunately, it also seems impossible to do out of bounds memory reads;
// reads before the string contents block seem to fail, and reads after the end of the file also seem to fail.
