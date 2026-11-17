// Chooses the pairs of chords to time from the results of the feasibility sweep. Writes the plan (the pairs, in
// order) and a deck holding every chord the plan uses, whose config must be loaded onto the Twiddler for timing.

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use itertools::Itertools;
use rand::rngs::StdRng;
use rand::SeedableRng;

use twiddler_keymap::chord::Chord;
use twiddler_keymap::deck::Deck;
use twiddler_keymap::results::read_records;
use twiddler_keymap::sweep::{outcomes, Record};
use twiddler_keymap::timing_plan::{plan, references};

#[derive(Parser)]
#[command(about = "Choose the pairs of chords to time, from the results of the feasibility sweep")]
struct Args {
    /// The results file written by the sweep
    sweep_results: PathBuf,
    /// Where to write timing_plan.json, timing_deck.cfg and timing_deck.json
    out_dir: PathBuf,
    /// Different seeds give different pairings and orders
    #[arg(long, default_value_t = 0)]
    seed: u64,
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let results = File::open(&args.sweep_results).map_err(|e| format!("failed to open {}: {}", args.sweep_results.display(), e))?;
    let outcomes = outcomes(&read_records::<Record>(BufReader::new(results))?);
    let trials = plan(&outcomes, &mut StdRng::seed_from_u64(args.seed))?;

    let chords: Vec<Chord> = trials.iter().flatten().copied().unique().sorted_by_key(|chord| (chord.n_buttons(), chord.button_mask())).collect();
    let deck = Deck::new(chords)?;
    let plan_path = args.out_dir.join("timing_plan.json");
    std::fs::write(&plan_path, serde_json::to_string(&trials)?)?;
    let [config_path, deck_path] = deck.write_files(&args.out_dir, "timing_deck")?;

    println!("reference chords:\n");
    for reference in references(&outcomes)? {
        println!("{}", reference);
    }
    println!("{}: {} trials over {} chords", plan_path.display(), trials.len(), deck.chords().len());
    println!("{} and {}: the deck for these trials", config_path.display(), deck_path.display());
    Ok(())
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
