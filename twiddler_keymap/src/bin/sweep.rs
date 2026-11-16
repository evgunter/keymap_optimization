// Runs the feasibility sweep for one deck: shows each chord in turn and records whether it can be pressed and how
// comfortable it is. Chords are pressed on the Twiddler; everything else is typed on another keyboard.
// Each result is appended to the results file as it is made, so the sweep can be stopped and resumed at any point.

use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::SystemTime;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, terminal};

use twiddler_keymap::deck::Deck;
use twiddler_keymap::reader::{KeystrokeReader, ReadError, TimedKeystroke};
use twiddler_keymap::sweep::{read_outcomes, write_record, Comfort, Command, Record, Response, Session, Step};

#[derive(Parser)]
#[command(about = "Record which chords of a deck can be pressed, and how comfortably")]
struct Args {
    /// The Twiddler's keyboard device, e.g. /dev/input/event5
    device: PathBuf,
    /// A deck file written by gen_sweep_decks; the config written with it must be loaded on the Twiddler
    deck: PathBuf,
    /// The file results are appended to; use the same one for every deck
    results: PathBuf,
    /// Also ask about chords that cannot be pressed without pressing a chord already marked as impossible
    #[arg(long)]
    include_supersets: bool,
}

enum Input {
    Twiddler(Result<Vec<TimedKeystroke>, ReadError>),
    Terminal(io::Result<Event>),
}

enum Action {
    Command(Command),
    Quit,
}

fn action_for(key: KeyEvent) -> Option<Action> {
    match key.code {
        // in raw mode the terminal does not turn ctrl-c into an interrupt
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('i') => Some(Action::Command(Command::CannotPress)),
        KeyCode::Char('n') => Some(Action::Command(Command::NotRegistered)),
        KeyCode::Char('u') => Some(Action::Command(Command::Undo)),
        KeyCode::Char('1') => Some(Action::Command(Command::Rate(Comfort::Comfortable))),
        KeyCode::Char('2') => Some(Action::Command(Command::Rate(Comfort::Ok))),
        KeyCode::Char('3') => Some(Action::Command(Command::Rate(Comfort::Awkward))),
        KeyCode::Char('4') => Some(Action::Command(Command::Rate(Comfort::Painful))),
        _ => None,
    }
}

fn message_for(response: Response) -> String {
    match response {
        Response::Moved | Response::Recorded(_, _) | Response::Ignored => String::new(),
        Response::WrongChord(Some(chord)) => format!("That registered as a different chord:\n{}", chord),
        Response::WrongChord(None) => "The Twiddler sent a keystroke that no chord in this deck sends. Is this deck's config loaded?".to_string(),
        Response::NothingToUndo => "There is nothing to go back to.".to_string(),
    }
}

fn screen(session: &Session, message: &str) -> String {
    let progress = session.progress();
    let status = format!("{} recorded, {} skipped because pressing them includes a chord you can't press, {} to go",
                         progress.recorded, progress.skipped, progress.remaining);
    let prompt = match session.step() {
        Step::Press(chord) => format!("Get the Twiddler to register this chord, in whichever way is easiest:\n\n{}\n\
                                       i  I can't press it\n\
                                       n  I pressed it, but it didn't register\n\
                                       u  go back to the previous chord\n\
                                       q  quit", chord),
        Step::Rate(chord) => format!("It registered. How comfortable was it?\n\n{}\n\
                                      1  comfortable\n\
                                      2  ok\n\
                                      3  awkward\n\
                                      4  painful\n\
                                      u  press it again\n\
                                      q  quit", chord),
        Step::Finished => "This deck is finished.\n\n\
                           u  go back to the previous chord\n\
                           q  quit".to_string(),
    };
    format!("{}\n\n{}\n\n{}\n", status, prompt, message)
}

fn render(text: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0))?;
    // in raw mode a line feed does not return the cursor to the start of the line
    write!(stdout, "{}", text.replace('\n', "\r\n"))?;
    stdout.flush()
}

/// Keeps the terminal in raw mode, in which keys are delivered as they are typed, until dropped.
struct RawMode;

impl RawMode {
    fn enable() -> io::Result<Self> {
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

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let deck_file = File::open(&args.deck).map_err(|e| format!("failed to open deck {}: {}", args.deck.display(), e))?;
    let deck: Deck = serde_json::from_reader(BufReader::new(deck_file))?;
    let outcomes = if args.results.exists() {
        read_outcomes(BufReader::new(File::open(&args.results)?))?
    } else {
        HashMap::new()
    };
    let mut results = OpenOptions::new().create(true).append(true).open(&args.results)
        .map_err(|e| format!("failed to open results {}: {}", args.results.display(), e))?;
    let mut reader = KeystrokeReader::open(&args.device)
        .map_err(|e| format!("failed to open device {}: {}", args.device.display(), e))?;
    let mut session = Session::new(deck, outcomes, !args.include_supersets);

    // the Twiddler and the terminal are each read on their own thread, since both block
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
        let event = event::read();
        let failed = event.is_err();
        if sender.send(Input::Terminal(event)).is_err() || failed {
            break;
        }
    });

    let _raw_mode = RawMode::enable()?;
    let mut message = String::new();
    loop {
        render(&screen(&session, &message))?;
        let responses: Vec<Response> = match receiver.recv()? {
            Input::Twiddler(keystrokes) => keystrokes?.into_iter().map(|timed| session.handle_keystroke(timed.keystroke)).collect(),
            Input::Terminal(event) => match event? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match action_for(key) {
                    Some(Action::Quit) => return Ok(()),
                    Some(Action::Command(command)) => vec![session.handle_command(command)],
                    None => Vec::new(),
                },
                _ => Vec::new(),
            },
        };
        for response in responses {
            if let Response::Recorded(chord, outcome) = response {
                write_record(&mut results, &Record { chord, outcome, time: SystemTime::now() })?;
            }
            message = message_for(response);
        }
    }
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
