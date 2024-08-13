use core::fmt;
use rand::distributions::{Distribution, Standard};
use strum::{EnumCount, VariantArray};
use std::marker::PhantomData;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

// This file contains definitions of the traits that need to be instantiated by a keyboard config, and the associated generic data structures.

pub trait Key: Sized + fmt::Display + PartialEq + Copy + EnumCount + VariantArray + fmt::Debug + Serialize + DeserializeOwned
where
    Standard: Distribution<Self>
{}

pub trait Layout<K: Key, const N: usize>: Sized + Serialize + DeserializeOwned where Standard: Distribution<K> {
    fn fmt_chord(chord: &Chord<K, N, Self>, f: &mut fmt::Formatter) -> fmt::Result;
}

// A combination of keys pressed simultaneously.
#[derive(PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(Debug)]
// N is the number of distinct keys that there are, i.e. Key::COUNT (which can't be used here since it's a generic)
pub struct Chord<K: Key, const N: usize, L: Layout<K, N>> where Standard: Distribution<K> {
    #[serde(with = "serde_arrays")]
    keys: [bool; N],
    #[serde(skip)]
    _marker0: PhantomData<K>,
    #[serde(skip)]
    _marker1: PhantomData<L>,
}

impl<K: Key, const N: usize, L: Layout<K, N>> Chord<K, N, L> where Standard: Distribution<K> {
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
}

impl<K: Key, const N: usize, L: Layout<K, N>> fmt::Display for Chord<K, N, L> where Standard: Distribution<K> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        L::fmt_chord(&self, f)
    }
}
