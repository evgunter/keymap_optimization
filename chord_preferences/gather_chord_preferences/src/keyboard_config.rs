use core::fmt;
use rand::distributions::{Distribution, Standard};
use strum::{EnumCount, VariantArray};
use std::marker::PhantomData;

// This file contains definitions of the traits that need to be instantiated by a keyboard config.
// TODO: remove Debug
pub trait Key: Sized + fmt::Display + PartialEq + Copy + EnumCount + VariantArray + fmt::Debug {}

impl<T> Key for T
where
    T: Sized + fmt::Display + PartialEq + Copy + EnumCount + VariantArray + fmt::Debug + Distribution<Standard>,
{}

pub trait Layout<K: Key, const N: usize>: Sized {
    fn fmt_chord(chord: &Chord<K, N, Self>, f: &mut fmt::Formatter) -> fmt::Result;
}

// A combination of keys pressed simultaneously.
#[derive(Debug)]
pub struct Chord<K: Key, const N: usize, L: Layout<K, N>> {  // N is the number of distinct keys that there are, i.e. Key::COUNT (which can't be used here since it's a generic)
    keys: [bool; N],
    _marker0: PhantomData<K>,
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
}

impl<K: Key, const N: usize, L: Layout<K, N>> fmt::Display for Chord<K, N, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        L::fmt_chord(&self, f)
    }
}
