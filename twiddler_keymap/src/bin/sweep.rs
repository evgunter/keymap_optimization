// Runs the feasibility sweep for one deck: shows each chord in turn and records whether it can be pressed and how
// comfortable it is. Chords are pressed on the Twiddler; everything else is typed on another keyboard.
// Each result is appended to the results file as it is made, so the sweep can be stopped and resumed at any point.

use std::collections::HashMap;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::SystemTime;

use clap::Parser;

use twiddler_keymap::console::{render, spawn_inputs, Input, RawMode};
use twiddler_keymap::deck::Deck;
use twiddler_keymap::reader::KeystrokeReader;
use twiddler_keymap::results::{append_record, read_records};
use twiddler_keymap::sweep::{outcomes, Comfort, Command, Record, Response, Session, Step};

#[derive(Parser)]
#[command(about = "Record which chords of a deck can be pressed, and how comfortably")]
struct Args {
    /// The Twiddler's keyboard device, e.g. /dev/input/event5
    device: PathBuf,
    /// A deck file written by gen_sweep_decks; the config written with it must be loaded on the Twiddler
    deck: PathBuf,
    /// The file results are appended to; use the same one for every deck
    results: PathBuf,
    /// Leave out chords that cannot be pressed without pressing a chord already marked as impossible.
    /// This can leave out chords that are pressable, since an extra button sometimes makes a shape easier
    #[arg(long)]
    skip_supersets: bool,
}

fn command_for(character: char) -> Option<Command> {
    match character {
        'i' => Some(Command::CannotPress),
        'n' => Some(Command::NotRegistered),
        'u' => Some(Command::Undo),
        '1' => Some(Command::Rate(Comfort::Comfortable)),
        '2' => Some(Command::Rate(Comfort::Ok)),
        '3' => Some(Command::Rate(Comfort::Awkward)),
        '4' => Some(Command::Rate(Comfort::Painful)),
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
    let skipped = match progress.skipped {
        0 => String::new(),
        n => format!("{} skipped because pressing them includes a chord you can't press, ", n),
    };
    let status = format!("{} recorded, {}{} to go", progress.recorded, skipped, progress.remaining);
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

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let deck_file = File::open(&args.deck).map_err(|e| format!("failed to open deck {}: {}", args.deck.display(), e))?;
    let deck: Deck = serde_json::from_reader(BufReader::new(deck_file))?;
    let outcomes = if args.results.exists() {
        outcomes(&read_records::<Record>(BufReader::new(File::open(&args.results)?))?)
    } else {
        HashMap::new()
    };
    let mut results = OpenOptions::new().create(true).append(true).open(&args.results)
        .map_err(|e| format!("failed to open results {}: {}", args.results.display(), e))?;
    let reader = KeystrokeReader::open(&args.device)
        .map_err(|e| format!("failed to open device {}: {}", args.device.display(), e))?;
    let mut session = Session::new(deck, outcomes, args.skip_supersets);

    let inputs = spawn_inputs(reader);
    let _raw_mode = RawMode::enable()?;
    let mut message = String::new();
    loop {
        render(&screen(&session, &message))?;
        let responses: Vec<Response> = match inputs.recv()? {
            Input::Quit => return Ok(()),
            Input::Twiddler(keystrokes) => keystrokes?.into_iter().map(|timed| session.handle_keystroke(timed.keystroke)).collect(),
            Input::Typed(character) => command_for(character?).map(|command| session.handle_command(command)).into_iter().collect(),
        };
        for response in responses {
            if let Response::Recorded(chord, outcome) = response {
                append_record(&mut results, &Record { chord, outcome, time: SystemTime::now() })?;
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
