use core::fmt;

// This file contains definitions of the traits that need to be instantiated by a keyboard config.
// TODO: remove Debug
pub trait Key: Sized + fmt::Display + fmt::Debug {}

pub trait Layout<K: Key> {
    fn display_chord(&self, chord: Chord<K>);
}

// A combination of keys pressed simultaneously.
#[derive(Debug)]
pub struct Chord<KeyType: Key> {
    pub keys: Vec<KeyType>,
}

impl<K: Key> fmt::Display for Chord<K> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.keys)
    }
}
