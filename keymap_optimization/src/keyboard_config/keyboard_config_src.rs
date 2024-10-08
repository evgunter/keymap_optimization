use core::fmt;
use strum::{EnumCount, VariantArray};
use std::marker::PhantomData;
use std::error::Error;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

// this file contains definitions of the traits that need to be instantiated by a keyboard config, and the associated generic data structures.

pub trait Key: Sized + fmt::Display + PartialEq + Copy + EnumCount + VariantArray + fmt::Debug + Serialize + DeserializeOwned
{
    fn gen_random<R: rand::Rng>(rng: &mut R) -> Self;
}

pub trait Layout<K: Key, const N: usize>: Sized + Serialize + DeserializeOwned + fmt::Debug + Clone + PartialEq {
    fn fmt_chord_graphical(chord: &Chord<K, N, Self>, f: &mut fmt::Formatter) -> fmt::Result;
    fn fmt_chord_text(chord: &Chord<K, N, Self>, f: &mut fmt::Formatter) -> fmt::Result;
    fn is_valid(chord: &Chord<K, N, Self>) -> bool;
}

// a combination of keys pressed simultaneously
#[derive(PartialEq, Clone)]
#[derive(Serialize, Deserialize)]
#[derive(Debug)]
// N is the number of distinct keys that there are, i.e. Key::COUNT (which can't be used here since it's a generic)
pub struct Chord<K: Key, const N: usize, L: Layout<K, N>> {
    #[serde(with = "serde_arrays")]
    keys: [bool; N],
    #[serde(skip)]
    _marker0: PhantomData<K>,
    #[serde(skip)]
    _marker1: PhantomData<L>,
}

impl<K: Key, const N: usize, L: Layout<K, N>> Chord<K, N, L> {
    pub fn new() -> Self {
        Self {
            keys: [false; N],
            _marker0: PhantomData,
            _marker1: PhantomData,
        }
    }

    fn index(&self, key: K) -> usize {
        K::VARIANTS.iter().position(|x| *x == key).unwrap()
    }

    pub fn contains(&self, key: K) -> bool {
        self.keys[self.index(key)]
    }

    pub fn add_key(&mut self, key: K) {
        self.keys[self.index(key)] = true;
    }

    pub fn n_keys(&self) -> usize {
        self.keys.iter().filter(|&&x| x).count()
    }

    pub fn to_vector(&self) -> Vec<bool> {
        self.keys.to_vec()
    }

    // allow direct editing of the private field .keys in the unit tests
    #[cfg(test)]
    pub(crate) fn get_raw_keys(&mut self) -> &mut [bool] {
        &mut self.keys
    }
}

pub struct GraphicalChord<'a, K: Key, const N: usize, L: Layout<K, N>> {
    pub chord: &'a Chord<K, N, L>,
}

impl<'a, K: Key, const N: usize, L: Layout<K, N>> fmt::Display for GraphicalChord<'a, K, N, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        L::fmt_chord_graphical(&self.chord, f)
    }
}

impl<K: Key, const N: usize, L: Layout<K, N>> fmt::Display for Chord<K, N, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        L::fmt_chord_text(&self, f)
    }
}

fn _display_chord_sequence<K: Key, const N: usize, L: Layout<K, N>>(chords: &Vec<Chord<K, N, L>>) -> String {
    chords.into_iter().map(|c| K::VARIANTS.iter()
                                          .filter(|key| c.contains(**key))
                                          .map(|key| format!("{}", key))
                                          .collect::<String>())
                      .collect::<Vec<String>>()
                      .join(" ")
}

pub trait ChordSampler<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng, I> where Self: Sized {
    fn new(rng: R, info: &I) -> Result<Self, Box<dyn Error>>;  // I is the initialization info
    fn sample_chord(&mut self) -> Chord<K, N, L>;  // this need not be uniform. there may be multiple samplers for the same type of chord
}

pub trait ChordTrialUtils<K: Key, const N: usize, L: Layout<K, N>, R: rand::Rng, I, S: ChordSampler<K, N, L, R, I>>: Sized + Serialize + DeserializeOwned {
    fn new(chord_sampler: S) -> Self;
    fn get_config(&self) -> Result<Vec<u8>, Box<dyn Error>>;
    fn get_vocab(&self) -> &Vec<(Chord<K, N, L>, String)>;
    fn parse_trial_string(&self, test_string: &str) -> Result<Vec<Chord<K, N, L>>, Box<dyn Error>>;
    // TODO: these are inefficient. if we ever need ok performance on this, should create and store a hashmap inside the struct
    fn lookup_chord(&self, chord: &Chord<K, N, L>) -> Option<String> {
        self.get_vocab().iter().find(|(c, _)| c == chord).map(|(_, s)| s.clone())
    }
    fn lookup_string(&self, string: &str) -> Option<Chord<K, N, L>> {
        self.get_vocab().iter().find(|(_, s)| s == string).map(|(c, _)| c.clone())
    }
}
