#![recursion_limit="2048"]

pub mod keyboard_config;
pub mod keyboard_config_implementations;
pub mod chord_preferences;

pub mod local_env;

#[cfg(test)]
#[macro_use] extern crate eager;
mod tests;

pub use keyboard_config_implementations::twiddler;
