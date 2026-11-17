//! Reads the keystrokes the Twiddler sends, with the times at which the host received them.

use std::io;
use std::path::Path;
use std::time::{Instant, SystemTime};

use evdev::{Device, EventSummary, SynchronizationCode};

use crate::decode::{DecodeError, KeystrokeDecoder};
use crate::keystroke::Keystroke;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TimedKeystroke {
    /// When the kernel received the keystroke. This is precise, but it is the system clock, which can be
    /// adjusted while we are measuring.
    pub time: SystemTime,
    /// When this program received the keystroke: later than `time` by a varying delay, but on a clock that
    /// only runs forwards, so it can be used to check intervals measured with `time`.
    pub received: Instant,
    pub keystroke: Keystroke,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("failed to read from the Twiddler: {0}")]
    Io(#[from] io::Error),
    #[error("unexpected input from the Twiddler: {0}")]
    Decode(#[from] DecodeError),
}

pub struct KeystrokeReader {
    device: Device,
    decoder: KeystrokeDecoder,
    // the key events (key code, value) received since the last SYN_REPORT
    report: Vec<(u16, i32)>,
}

impl KeystrokeReader {
    /// Opens a Linux input device (`/dev/input/event*`) and grabs it, so that its keystrokes reach only this
    /// program for as long as the reader exists.
    pub fn open(device_path: &Path) -> io::Result<Self> {
        let mut device = Device::open(device_path)?;
        device.grab()?;
        Ok(KeystrokeReader { device, decoder: KeystrokeDecoder::default(), report: Vec::new() })
    }

    /// Waits until the device sends events, then returns the keystrokes among them (possibly none).
    pub fn read(&mut self) -> Result<Vec<TimedKeystroke>, ReadError> {
        let mut keystrokes = Vec::new();
        let events = self.device.fetch_events()?;
        let received = Instant::now();
        for event in events {
            let time = event.timestamp();
            match event.destructure() {
                EventSummary::Key(_, key, value) => self.report.push((key.code(), value)),
                EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) => {
                    let report = std::mem::take(&mut self.report);
                    if let Some(keystroke) = self.decoder.evdev_report(&report)? {
                        keystrokes.push(TimedKeystroke { time, received, keystroke });
                    }
                }
                _ => (),
            }
        }
        Ok(keystrokes)
    }
}
