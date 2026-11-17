// Runs timing trials following a plan: for each pair of chords, a timed run of alternating between them.
// Chords are pressed on the Twiddler; everything else is typed on another keyboard.
// Each run is appended to the results file as it ends, so a session can be stopped and resumed at any point.

use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use unicode_width::UnicodeWidthStr;

use twiddler_keymap::chord::Chord;
use twiddler_keymap::console::{render, spawn_inputs, Input, RawMode};
use twiddler_keymap::deck::Deck;
use twiddler_keymap::reader::KeystrokeReader;
use twiddler_keymap::results::{append_record, read_records};
use twiddler_keymap::timing::{median, Attempt, Command, Response, Session, Step, Summary, PRESSES_PER_RUN};

#[derive(Parser)]
#[command(about = "Time alternating between the pairs of chords in a plan")]
struct Args {
    /// The Twiddler's keyboard device, e.g. /dev/input/event5
    device: PathBuf,
    /// The deck file written by gen_timing_plan; the config written with it must be loaded on the Twiddler
    deck: PathBuf,
    /// The plan file written by gen_timing_plan
    plan: PathBuf,
    /// The file results are appended to; use one results file per plan
    results: PathBuf,
}

fn command_for(character: char) -> Option<Command> {
    match character {
        ' ' => Some(Command::Proceed),
        'r' => Some(Command::Redo),
        _ => None,
    }
}

/// Draws the two chords next to each other, the first on the left.
fn side_by_side(pair: &[Chord; 2]) -> String {
    let [left, right] = pair.map(|chord| chord.to_string());
    let width = left.lines().map(|line| line.width()).max().unwrap_or(0);
    left.lines().zip(right.lines())
        .map(|(left_line, right_line)| format!("{}{}      {}\n", left_line, " ".repeat(width - left_line.width()), right_line))
        .collect()
}

fn message_for(response: &Response, pair: Option<[Chord; 2]>) -> String {
    match response {
        Response::Moved | Response::Ended(_) | Response::Ignored => String::new(),
        Response::Practised(None) => "The Twiddler sent a keystroke that no chord in this deck sends. Is this deck's config loaded?".to_string(),
        Response::Practised(Some(chord)) => match pair.map(|pair| pair.iter().position(|expected| expected == chord)) {
            Some(Some(0)) => "That was the left chord.".to_string(),
            Some(Some(_)) => "That was the right chord.".to_string(),
            Some(None) | None => format!("That registered as a different chord:\n{}", chord),
        },
        Response::ClockUnsteady => "The system clock changed during that run, so its times can't be trusted and it was thrown away.".to_string(),
    }
}

fn describe_switches(label: &str, seconds: &[f64]) -> String {
    match median(seconds) {
        Some(median) => format!("{}: median {:.0} ms over {} correct switches", label, median * 1000.0, seconds.len()),
        None => format!("{}: no correct switches", label),
    }
}

fn describe(summary: &Summary) -> String {
    format!("{}\n{}\n{}",
            if summary.exact { "Every chord was the right one." } else { "Not every chord was the right one." },
            describe_switches("left to right", &summary.switches[0]),
            describe_switches("right to left", &summary.switches[1]))
}

fn pair_of(step: &Step) -> Option<[Chord; 2]> {
    match step {
        Step::Practice { pair, .. } | Step::Run { pair, .. } | Step::Review { pair, .. } => Some(*pair),
        Step::Finished => None,
    }
}

fn screen(session: &Session, message: &str) -> String {
    let body = match session.step() {
        Step::Practice { trial, pair } => format!(
            "Trial {} of {}\n\n\
             Alternate between these two chords, starting with the left one:\n\n{}\n\
             Practise as much as you like. The timed run is {} chords, and its clock starts with your first chord.\n\n\
             space  start the timed run\n\
             q      quit",
            trial + 1, session.n_trials(), side_by_side(&pair), PRESSES_PER_RUN),
        Step::Run { trial, pair, n_presses } => format!(
            "Trial {} of {}: timed run, {} of {} chords\n\n{}\n\
             r  give up this run and go back to practising\n\
             q  quit",
            trial + 1, session.n_trials(), n_presses, PRESSES_PER_RUN, side_by_side(&pair)),
        Step::Review { trial, pair, summary } => format!(
            "Trial {} of {}: run complete\n\n{}\n{}\n\n\
             space  keep this run and go on\n\
             r      redo it (this run is saved, marked as redone)\n\
             q      quit",
            trial + 1, session.n_trials(), side_by_side(&pair), describe(&summary)),
        Step::Finished => "The plan is finished.\n\nq  quit".to_string(),
    };
    format!("{}\n\n{}\n", body, message)
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let open = |path: &PathBuf| File::open(path).map(BufReader::new).map_err(|e| format!("failed to open {}: {}", path.display(), e));
    let deck: Deck = serde_json::from_reader(open(&args.deck)?)?;
    let plan: Vec<[Chord; 2]> = serde_json::from_reader(open(&args.plan)?)?;
    let attempts: Vec<Attempt> = if args.results.exists() { read_records(open(&args.results)?)? } else { Vec::new() };
    let mut session = Session::new(deck, plan, &attempts)?;
    let mut results = OpenOptions::new().create(true).append(true).open(&args.results)
        .map_err(|e| format!("failed to open results {}: {}", args.results.display(), e))?;
    let reader = KeystrokeReader::open(&args.device)
        .map_err(|e| format!("failed to open device {}: {}", args.device.display(), e))?;

    let inputs = spawn_inputs(reader);
    let _raw_mode = RawMode::enable()?;
    let mut message = String::new();
    loop {
        render(&screen(&session, &message))?;
        let pair = pair_of(&session.step());
        let responses: Vec<Response> = match inputs.recv()? {
            Input::Quit => return Ok(()),
            Input::Twiddler(keystrokes) => keystrokes?.into_iter().map(|timed| session.handle_keystroke(timed)).collect(),
            Input::Typed(character) => command_for(character?).map(|command| session.handle_command(command)).into_iter().collect(),
        };
        for response in responses {
            if let Response::Ended(attempt) = &response {
                append_record(&mut results, attempt)?;
            }
            message = message_for(&response, pair);
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

#[cfg(test)]
mod tests {
    use super::*;
    use twiddler_keymap::chord::{FingerState::{Lifted, L, LMR, R}, ThumbState};

    #[test]
    fn chords_are_drawn_next_to_each_other_in_aligned_columns() {
        let pair = [
            Chord::new(ThumbState::Num, [L, Lifted, Lifted, LMR]).unwrap(),
            Chord::new(ThumbState::Shift, [Lifted, R, Lifted, Lifted]).unwrap(),
        ];
        let drawn = side_by_side(&pair);
        assert_eq!(drawn, "⚫⚪⚪⚪      ⚪⚪⚪⚫\n ⚫⚪⚪        ⚪⚪⚪\n ⚪⚪⚪        ⚪⚪⚫\n ⚪⚪⚪        ⚪⚪⚪\n ⚫⚫⚫        ⚪⚪⚪\n");
        // the right chord is drawn from the same column on every line
        let right = pair[1].to_string();
        assert!(drawn.lines().zip(right.lines()).all(|(line, right_line)| line.ends_with(right_line) && line.width() - right_line.width() == 14));
    }
}
