// Chooses the pairs of chords to time from the results of the feasibility sweep. Writes the plan (the pairs, in
// random order) and the deck to load onto the Twiddler for timing, which identifies every chord the Twiddler
// can see, so that mistakes can be recognised whatever they are.

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use rand::rngs::StdRng;
use rand::SeedableRng;

use twiddler_keymap::deck::Deck;
use twiddler_keymap::results::read_records;
use twiddler_keymap::sweep::{outcomes, Record};
use twiddler_keymap::timing_plan::{plan, references};

#[derive(Parser)]
#[command(about = "Choose the pairs of chords to time, from the results of the feasibility sweep")]
struct Args {
    /// Where to write timing_plan.json, timing_deck.cfg and timing_deck.json
    out_dir: PathBuf,
    /// The results files written by the sweep. Where files disagree about a chord, the last one counts
    #[arg(required = true)]
    sweep_results: Vec<PathBuf>,
    /// Different seeds give different pairings and orders
    #[arg(long, default_value_t = 0)]
    seed: u64,
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    if !args.out_dir.is_dir() {
        return Err(format!("{} is not a directory; the output directory comes first, then the sweep results", args.out_dir.display()).into());
    }
    let mut records: Vec<Record> = Vec::new();
    for path in &args.sweep_results {
        let results = File::open(path).map_err(|e| format!("failed to open {}: {}", path.display(), e))?;
        records.extend(read_records::<Record>(BufReader::new(results)).map_err(|e| format!("{}: {}", path.display(), e))?);
    }
    let outcomes = outcomes(&records);
    let trials = plan(&outcomes, &mut StdRng::seed_from_u64(args.seed))?;

    let deck = Deck::full();
    let plan_path = args.out_dir.join("timing_plan.json");
    std::fs::write(&plan_path, serde_json::to_string(&trials)?)?;
    let [config_path, deck_path] = deck.write_files(&args.out_dir, "timing_deck")?;

    println!("reference chords:\n");
    for reference in references(&outcomes)? {
        println!("{}", reference);
    }
    println!("{}: {} trials", plan_path.display(), trials.len());
    println!("{} and {}: the deck for timing, which identifies every chord", config_path.display(), deck_path.display());
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
