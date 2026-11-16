// Writes the decks for the feasibility sweep: every chord the Twiddler can see that does not use the thumb,
// fewest buttons first.
// For each deck this writes a config to load onto the Twiddler and a JSON file listing the deck's chords.

use std::path::Path;

use twiddler_keymap::chord::{Chord, ThumbState};
use twiddler_keymap::deck::{Deck, MAX_CHORDS};

fn main() {
    let out_dir = std::env::args().nth(1).expect("usage: gen_sweep_decks <output directory>");
    let out_dir = Path::new(&out_dir);

    let mut chords: Vec<Chord> = Chord::all().filter(|chord| chord.thumb() == ThumbState::Lifted && chord.registers_as_itself()).collect();
    // a stable sort, so that chords with equally many buttons keep a fixed order
    chords.sort_by_key(|chord| chord.n_buttons());

    for (index, deck_chords) in chords.chunks(MAX_CHORDS).enumerate() {
        let deck = Deck::new(deck_chords.to_vec()).expect("chunks of distinct chords are valid decks");
        let config_path = out_dir.join(format!("sweep_deck_{}.cfg", index));
        let deck_path = out_dir.join(format!("sweep_deck_{}.json", index));
        std::fs::write(&config_path, deck.config()).expect("failed to write config");
        std::fs::write(&deck_path, serde_json::to_string(&deck).expect("failed to serialize deck")).expect("failed to write deck");

        let n_buttons = deck_chords.iter().map(|chord| chord.n_buttons());
        println!("{} and {}: {} chords with {} to {} buttons", config_path.display(), deck_path.display(),
                 deck_chords.len(), n_buttons.clone().min().unwrap(), n_buttons.max().unwrap());
    }
}
