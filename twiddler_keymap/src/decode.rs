//! Turns the key events the host receives from the Twiddler back into keystrokes.

use keycode::{KeyMap, KeyMapping};

use crate::keystroke::Keystroke;

/// A key as the Twiddler identifies it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HidKey {
    /// A modifier key, as its bit in the USB HID modifier bitmap.
    Modifier(u8),
    /// Any other key, as its USB HID keyboard usage id.
    Key(u8),
}

impl HidKey {
    /// The key that Linux reports with the given evdev key code, if it is a keyboard key.
    pub fn from_evdev(code: u16) -> Option<HidKey> {
        let key_map = KeyMap::from_key_mapping(KeyMapping::Evdev(code)).ok()?;
        match key_map.modifier {
            Some(modifier) => Some(HidKey::Modifier(modifier.bits())),
            None => u8::try_from(key_map.usb).ok().map(HidKey::Key),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyChange {
    Press,
    Release,
}

#[derive(Debug, PartialEq, thiserror::Error)]
#[error("more than one non-modifier key was pressed at once, but each chord sends a single key")]
pub struct MultipleKeysPressed;

/// Tracks which modifier keys are held, to recover the keystrokes sent by the Twiddler.
#[derive(Default)]
pub struct KeystrokeDecoder {
    modifiers: u8,
}

impl KeystrokeDecoder {
    /// Takes the key changes that the host received together (in evdev, the events up to a `SYN_REPORT`)
    /// and returns the keystroke they start, if any.
    pub fn report(&mut self, changes: &[(HidKey, KeyChange)]) -> Result<Option<Keystroke>, MultipleKeysPressed> {
        // changes that arrive together describe a single state of the keyboard, so their order carries no meaning:
        // the modifiers that apply to a key are the ones held once all the changes have been applied
        let mut pressed_keys = Vec::new();
        for (key, change) in changes {
            match (key, change) {
                (HidKey::Modifier(bit), KeyChange::Press) => self.modifiers |= bit,
                (HidKey::Modifier(bit), KeyChange::Release) => self.modifiers &= !bit,
                (HidKey::Key(key), KeyChange::Press) => pressed_keys.push(*key),
                (HidKey::Key(_), KeyChange::Release) => (),
            }
        }
        match pressed_keys[..] {
            [] => Ok(None),
            [key] => Ok(Some(Keystroke { modifiers: self.modifiers, key })),
            _ => Err(MultipleKeysPressed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyChange::{Press, Release};

    const L_CTRL: HidKey = HidKey::Modifier(0x01);
    const R_SHIFT: HidKey = HidKey::Modifier(0x20);
    const A: HidKey = HidKey::Key(0x04);
    const B: HidKey = HidKey::Key(0x05);

    #[test]
    fn evdev_codes_convert_to_hid_keys() {
        // evdev codes from linux/input-event-codes.h
        assert_eq!(HidKey::from_evdev(30), Some(A));  // KEY_A
        assert_eq!(HidKey::from_evdev(11), Some(HidKey::Key(0x27)));  // KEY_0
        assert_eq!(HidKey::from_evdev(29), Some(L_CTRL));  // KEY_LEFTCTRL
        assert_eq!(HidKey::from_evdev(54), Some(R_SHIFT));  // KEY_RIGHTSHIFT
        assert_eq!(HidKey::from_evdev(126), Some(HidKey::Modifier(0x80)));  // KEY_RIGHTMETA
        assert_eq!(HidKey::from_evdev(0x110), None);  // BTN_LEFT
    }

    #[test]
    fn plain_key_is_a_keystroke_without_modifiers() {
        let mut decoder = KeystrokeDecoder::default();
        assert_eq!(decoder.report(&[(A, Press)]), Ok(Some(Keystroke { modifiers: 0, key: 0x04 })));
        assert_eq!(decoder.report(&[(A, Release)]), Ok(None));
    }

    #[test]
    fn modifiers_in_the_same_report_apply_in_either_order() {
        let expected = Ok(Some(Keystroke { modifiers: 0x21, key: 0x04 }));
        assert_eq!(KeystrokeDecoder::default().report(&[(L_CTRL, Press), (R_SHIFT, Press), (A, Press)]), expected);
        assert_eq!(KeystrokeDecoder::default().report(&[(A, Press), (R_SHIFT, Press), (L_CTRL, Press)]), expected);
    }

    #[test]
    fn modifiers_held_from_an_earlier_report_apply() {
        let mut decoder = KeystrokeDecoder::default();
        assert_eq!(decoder.report(&[(L_CTRL, Press)]), Ok(None));
        assert_eq!(decoder.report(&[(A, Press)]), Ok(Some(Keystroke { modifiers: 0x01, key: 0x04 })));
    }

    #[test]
    fn released_modifiers_no_longer_apply() {
        let mut decoder = KeystrokeDecoder::default();
        decoder.report(&[(L_CTRL, Press), (R_SHIFT, Press), (A, Press)]).unwrap();
        assert_eq!(decoder.report(&[(A, Release), (L_CTRL, Release)]), Ok(None));
        assert_eq!(decoder.report(&[(B, Press)]), Ok(Some(Keystroke { modifiers: 0x20, key: 0x05 })));
    }

    #[test]
    fn two_keys_at_once_are_an_error() {
        assert_eq!(KeystrokeDecoder::default().report(&[(A, Press), (B, Press)]), Err(MultipleKeysPressed));
    }
}
