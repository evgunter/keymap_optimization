//! What the interactive programs share: merging input from the Twiddler and the terminal, and drawing the screen.

use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, terminal};

use crate::reader::{KeystrokeReader, ReadError, TimedKeystroke};

pub enum Input {
    /// Keystrokes sent by the Twiddler.
    Twiddler(Result<Vec<TimedKeystroke>, ReadError>),
    /// A character typed in the terminal.
    Typed(io::Result<char>),
    /// The request to quit, typed in the terminal as `q` or ctrl-c.
    Quit,
}

fn terminal_input(key: KeyEvent) -> Option<Input> {
    match key.code {
        // in raw mode the terminal does not turn ctrl-c into an interrupt
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Input::Quit),
        KeyCode::Char('q') => Some(Input::Quit),
        KeyCode::Char(character) => Some(Input::Typed(Ok(character))),
        _ => None,
    }
}

/// Starts reading the Twiddler and the terminal, each on its own thread since both block.
pub fn spawn_inputs(mut reader: KeystrokeReader) -> Receiver<Input> {
    let (sender, receiver) = mpsc::channel();
    let twiddler_sender = sender.clone();
    std::thread::spawn(move || loop {
        let keystrokes = reader.read();
        let failed = keystrokes.is_err();
        if twiddler_sender.send(Input::Twiddler(keystrokes)).is_err() || failed {
            break;
        }
    });
    std::thread::spawn(move || loop {
        let input = match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => terminal_input(key),
            Ok(_) => None,
            Err(e) => Some(Input::Typed(Err(e))),
        };
        let failed = matches!(input, Some(Input::Typed(Err(_))));
        if input.is_some_and(|input| sender.send(input).is_err()) || failed {
            break;
        }
    });
    receiver
}

/// Replaces what is on the screen with `text`.
pub fn render(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0))?;
    // in raw mode a line feed does not return the cursor to the start of the line
    write!(stdout, "{}", text.replace('\n', "\r\n"))?;
    stdout.flush()
}

/// Keeps the terminal in raw mode, in which keys are delivered as they are typed, until dropped.
pub struct RawMode;

impl RawMode {
    pub fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(RawMode)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        // there is nowhere to report a failure to restore the terminal
        let _ = terminal::disable_raw_mode();
    }
}
