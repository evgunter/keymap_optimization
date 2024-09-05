#![cfg(test)]

use crate::keyboard_config::{Chord, ChordTrialUtils, GraphicalChord};
use crate::twiddler::{TwiddlerLayout, TwiddlerKey, TwiddlerChord, TwiddlerChordTrialUtils, random_chord_, chord_list_to_config_object, Node, USB_HID_COUNT, RESERVED};
use crate::chord_preferences::logic::{TrialResults, TrialData, ErrCode, align, best_candidate, Direction};
use crate::chord_preferences::data_collection_keymap_gen::gen_random_config_with_trial_decoder;
use twidlk_rust::{generate_text_config, read_config};
use rand::Rng;
use strum::{EnumCount, VariantArray};

macro_rules! run_n_times {
    ($n:literal, $(#[$meta:meta])* $vis:vis fn $name:ident$(<$($($gen_arg:ident)*: $gen_trait:path),*>)?($($arg:ident: $typ:ty),*) $(-> $ret:ty)? $(where $($b:path: $d:path),*)? $body:block) => {
        $(#[$meta])*
        $vis fn $name$(<$($($gen_arg)*: $gen_trait),*>)?($($arg: $typ),*) $(-> $ret)? $(where $($b: $d),*)? {
            for _ in 0..$n {
                $body
            }
        }
    };
}

fn make_demo_trial<R: Rng> (rng: &mut R, threshold: f64, impossible_threshold: f64) -> TrialData<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> {
    let n_repetitions_per_trial = rng.gen_range(1..10);  // this will actually be fixed in practice, but doesn't hurt to vary it here
    // sometimes get a set of chord input randomly sampled to resemble the expected chords;
    // sometimes use ErrCode::Impossible
    let chord_pair = [random_chord_(rng, threshold), random_chord_(rng, threshold)];
    let trial_input = {
        if rng.gen::<f64>() < impossible_threshold {
            Err(ErrCode::Impossible)
        } else {
            let del_prob = 0.1;
            let ins_prob = 0.1;
            let sub_prob = 0.1;
            let mut input = Vec::new();
            for i in 0..2*n_repetitions_per_trial {
                if rng.gen::<f64>() > del_prob {  // < del_prob is a  deletion--don't add any input chord corresponding to this expected chord
                    loop {  // insert a geometric distribution number of random chords
                        if rng.gen::<f64>() < ins_prob {
                            input.push(random_chord_(rng, threshold));
                        } else {
                            break;
                        }
                    }
                    // insert the chord corresponding to the expected chord, perhaps with an error
                    if rng.gen::<f64>() < sub_prob {
                        input.push(random_chord_(rng, threshold));
                    } else {
                        input.push(chord_pair[i % 2].clone());
                    }
                }
            }
            Ok(input)
        }
    };
    
    TrialData {
        chord_pair,
        n_repetitions: n_repetitions_per_trial,
        input: trial_input,
    }
}

fn make_demo_data<R: Rng>(rng: &mut R, n_trials: usize, threshold: f64, impossible_threshold: f64) -> TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> {
    let mut demo_results = TrialResults::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>::new();
    for _ in 0..n_trials {
        demo_results.data.push(make_demo_trial(rng, threshold, impossible_threshold));
    }
    demo_results
}

fn make_demo_data_default() -> TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> {
    const THRESHOLD: f64 = 0.8;
    const IMPOSSIBLE_THRESHOLD: f64 = 0.2;
    let mut rng = rand::thread_rng();
    let n_trials = rng.gen_range(0..5);
    make_demo_data(&mut rng, n_trials, THRESHOLD, IMPOSSIBLE_THRESHOLD)
}

fn get_tmp_results_path(unique_id: &str) -> String {
    // it's important to have the files have different names--tests are run concurrently!
    // unique_id should generally just be the name of the function, unless multiple files are used in the same function.
    format!("/tmp/chord_preferences_results_{}_{}.json", unique_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
}

run_n_times!{10,
#[test]
fn serialization_round_trip_success() {
    // write some demo results to file, then load them from file and verify that they are identical.
    let results_path = get_tmp_results_path("serialization_round_trip_success");
    let demo_results = make_demo_data_default();

    println!("tmp file path: {}", results_path);

    match demo_results.save(&results_path) {
        Ok(_) => println!("Results saved successfully."),
        Err(e) => return assert!(false, "Error saving results: {}", e)
    }

    // now load the results and verify that they are the same as the originals
    let loaded_results = match TrialResults::load(&results_path) {
        Ok(loaded_results) => loaded_results,
        Err(e) => return assert!(false, "Error loading results: {}", e)
    };

    assert_eq!(loaded_results, demo_results)
}
}

fn serialization_round_trip_chord_edited(unique_id: &str, edit_fn: fn(usize, &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, &mut rand::prelude::ThreadRng) -> Result<(), &'static str>) {
    // this function is used for tests which edit results and check that the results indeed are detected as different.
    // they edit results in the following ways:
    // (a) add a new trial at a random position;
    // (b) remove a random trial;
    // (c) flip a random key in a random chord;
    // (d) change n_repetitions in a random trial
    // (e) change performance (time or accuracy) in a random trial

    // note that only (a) can be done if there are no trials. all the other tests will just report success in this case.
    
    // write some demo results to file, then load them from file and verify that they are identical.
    let results_path = get_tmp_results_path(unique_id);
    let mut demo_results = make_demo_data_default();

    match demo_results.save(&results_path) {
        Ok(_) => println!("Results saved successfully."),
        Err(e) => return assert!(false, "Error saving results: {}", e)
    }

    let mut rng = rand::thread_rng();
    let idx = if demo_results.data.is_empty() { 0 } else { rng.gen_range(0..demo_results.data.len()) };
    match edit_fn(idx, &mut demo_results, &mut rng) {
        Err(_) => return,  // the edit function can't be applied; treat this as a success
        _ => ()
    }

    // now load the results and verify that they are NOT the same as our edited results
    let loaded_results = match TrialResults::load(&results_path) {
        Ok(loaded_results) => loaded_results,
        Err(e) => return assert!(false, "Error loading results: {}", e)
    };

    assert!(loaded_results != demo_results);
}

run_n_times!{10,
#[test]
fn serialization_round_trip_add_trial() {
    // check that adding a new trial at a random position does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        demo_results.data.insert(idx, make_demo_trial(rng, 0.8, 0.2));
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_add_trial", edit_fn);
}
}

run_n_times!{10,
#[test]
fn serialization_round_trip_remove_trial() {
    // check that removing a random trial does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, _rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }
        demo_results.data.remove(idx);
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_remove_trial", edit_fn);
}
}

run_n_times!{10,
#[test]
fn serialization_round_trip_flip_key() {
    // check that flipping a random key in a random chord does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }
        let chord_idx = rng.gen_range(0..2);
        let key_idx = rng.gen_range(0..TwiddlerKey::COUNT);
        let chord_keys = &mut demo_results.data[idx].chord_pair[chord_idx].get_raw_keys();
        chord_keys[key_idx] = !chord_keys[key_idx];
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_flip_key", edit_fn);
}
}

run_n_times!{10,
#[test]
fn serialization_round_trip_change_repetitions() {
    // check that changing n_repetitions in a random trial does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, _rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }
        demo_results.data[idx].n_repetitions += 1;
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_change_repetitions", edit_fn);
}
}

run_n_times!{10,
#[test]
fn serialization_round_trip_toggle_input_error() {
    // check that switching input between an error and a result does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, _rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }
        demo_results.data[idx].input = match demo_results.data[idx].input {
            Ok(_) => Err(ErrCode::Impossible),
            Err(ErrCode::Impossible) => Ok(Vec::new()),
        };
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_toggle_input_error", edit_fn);
}
}

run_n_times!{100,
#[test]
fn serialization_round_trip_change_input() {
    // check that changing chords in a random trial does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }

        match &mut demo_results.data[idx].input {
            Ok(v) => {
                println!("editing input [{}]", v.clone().into_iter().map(|c| format!("{}", c)).collect::<Vec<String>>().join(", "));

                let sp_gen = if v.is_empty() || rng.gen::<f64>() < 0.5 {
                    println!("using insert");
                    fn sp_gen<R: Rng>(vlen: usize, rng: &mut R) -> (std::ops::Range<usize>, Vec<TwiddlerChord>) {
                        let random_index = rng.gen_range(0..vlen+1);
                        (random_index..random_index, vec![random_chord_(rng, 0.8)])
                    }
                    sp_gen
                } else {
                    println!("using delete on results of length {}", v.len());
                    fn sp_gen<R: Rng>(vlen: usize, rng: &mut R) -> (std::ops::Range<usize>, Vec<TwiddlerChord>) {
                        if vlen == 0 {
                            return (0..0, vec![]);
                        }
                        let random_index = rng.gen_range(0..vlen);
                        (random_index..random_index+1, vec![])
                    }
                    sp_gen
                };

                loop {
                    let (range, slice) = sp_gen(v.len(), rng);
                    if slice.is_empty() {
                        println!("deleting range {}-{}", range.start, range.end);
                    } else if range.start == range.end {
                        println!("inserting at index {}: [{}]", range.start, slice.clone().into_iter().map(|c| format!("{}", c)).collect::<Vec<String>>().join(", "));
                    } else {
                        println!("replacing range {}-{} with [{}]", range.start, range.end, slice.clone().into_iter().map(|c| format!("{}", c)).collect::<Vec<String>>().join(", "));
                    }
                    v.splice(range, slice);
                    if rng.gen::<f64>() < 0.5 {
                        break;
                    }
                }

                println!("edited input [{}]", v.clone().into_iter().map(|c| format!("{}", c)).collect::<Vec<String>>().join(", "));
            },
        
            Err(ErrCode::Impossible) => {  // toggling between error and result is specifically tested above
                println!("input was error");
                demo_results.data[idx].input = Ok(vec![])
              },
        };
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_change_time", edit_fn);
}
}

#[test]
fn empty_chord_display_graphical() {
    // check that displaying an empty chord does not panic
    let empty_chord: TwiddlerChord = Chord::new();
    println!("Empty chord: {}", GraphicalChord { chord: &empty_chord });
}

#[test]
fn full_chord_display_graphical() {
    // check that displaying a full chord does not panic
    let mut full_chord: TwiddlerChord = Chord::new();
    for key in TwiddlerKey::VARIANTS.iter() {
        full_chord.add_key(*key);
    }
    println!("Full chord: {}", GraphicalChord { chord: &full_chord });
}

run_n_times!{100,
#[test]
fn regular_chord_display_graphical() {
    // check that displaying a random chord does not panic
    let regular_chord: TwiddlerChord = random_chord_(&mut rand::thread_rng(), 0.8);
    println!("Regular chord: {}", GraphicalChord { chord: &regular_chord });
}
}

#[test]
fn empty_chord_display_text() {
    // check that displaying an empty chord does not panic
    let empty_chord: TwiddlerChord = Chord::new();
    println!("Empty chord: {}", empty_chord);
}

#[test]
fn full_chord_display_text() {
    // check that displaying a full chord does not panic
    let mut full_chord: TwiddlerChord = Chord::new();
    for key in TwiddlerKey::VARIANTS.iter() {
        full_chord.add_key(*key);
    }
    println!("Full chord: {}", full_chord);
}

run_n_times!{100,
    #[test]
    fn random_chord_display_text() {
        // check that displaying a random chord does not panic
        let random_chord: TwiddlerChord = random_chord_(&mut rand::thread_rng(), 0.8);
        println!("Regular chord: {}", random_chord);
    }
}
    

#[test]
fn index_usb_hid_conversion() {
    // check that the conversion functions are inverses of each other
    for i in 0..USB_HID_COUNT {
        let (shifted, usb) = Node::idx_to_usb(i).unwrap();
        let idx = Node::usb_to_idx(shifted, usb).unwrap();
        assert_eq!(i, idx);
    }
}

run_n_times!{10,
#[test]
fn make_config_and_decoder() {
    match gen_random_config_with_trial_decoder::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, TwiddlerChordTrialUtils>() {
        Ok(_) => (),
        Err(e) => assert!(false, "Error generating config: {}", e)
    }
}
}

run_n_times!{10,
#[test]
fn config_round_trip() {
    let (config_bin, chord_trial_utils) = gen_random_config_with_trial_decoder::<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout, TwiddlerChordTrialUtils>().unwrap();
    let twidlk_config = chord_list_to_config_object(chord_trial_utils.get_vocab().clone()).unwrap();
    let original_text_config = generate_text_config(&twidlk_config).unwrap();
    println!("original config:\n{}", original_text_config);

    let roundtrip_twidlk_config = read_config(&config_bin).unwrap();
    let roundtrip_text_config = generate_text_config(&roundtrip_twidlk_config).unwrap();
    println!("roundtrip config:\n{}", roundtrip_text_config);
    assert_eq!(original_text_config, roundtrip_text_config);
}
}

#[test]
fn empty_chord_is_invalid() {
    let chord: TwiddlerChord = Chord::new();
    assert!(!chord.is_valid());
}

run_n_times!{100,
#[test]
fn thumb_chord_is_invalid() {
    let mut rng = rand::thread_rng();
    let mut chord: TwiddlerChord = Chord::new();
    // choose 1-4 thumb keys at random
    let n_thumb_keys = rng.gen_range(1..5);
    for _ in 0..n_thumb_keys {
        chord.add_key(TwiddlerLayout::THUMB[rng.gen_range(0..TwiddlerLayout::THUMB.len())]);
    }
    assert!(!chord.is_valid());
}
}

fn reserved_to_tw() -> Vec<TwiddlerChord> {
    let mut reserved_as_tw_chords = Vec::new();
    for reserved_chord in RESERVED {
        let mut chord = TwiddlerChord::new();
        for key in reserved_chord {
            chord.add_key(key);
        }
        reserved_as_tw_chords.push(chord);
    }
    reserved_as_tw_chords
}

#[test]
fn reserved_chords_are_invalid() {
    for reserved_chord in reserved_to_tw() {
        assert!(!reserved_chord.is_valid());
    }
}

#[test]
fn reserved_chords_can_be_made_valid() {
    let mut rng = rand::thread_rng();
    let reserved_as_tw_chords = reserved_to_tw();
    for reserved_chord in reserved_as_tw_chords {
        let mut new_chord = reserved_chord.clone();
        loop {
            let key = TwiddlerKey::VARIANTS[rng.gen_range(0..TwiddlerKey::COUNT)];
            if !reserved_chord.contains(key) {
                new_chord.add_key(key);
                break;
            }
        }
        assert!(new_chord.is_valid());
    }
}

run_n_times!{1000,
#[test]
fn finger_chord_is_valid() {
    let mut rng = rand::thread_rng();
    // get a random starting chord
    let mut chord: TwiddlerChord = {
        if rng.gen::<f64>() < 0.1 {
            Chord::new()
        } else {
            random_chord_(&mut rng, 0.8)
        }
    };

    let non_thumb = TwiddlerLayout::MAIN.concat();
    loop {
        chord.add_key(non_thumb[rng.gen_range(0..non_thumb.len())]);
        if !reserved_to_tw().to_vec().contains(&chord) {
            break;
        }
    }
    assert!(chord.is_valid());
}
}

fn print_dirn_matrix<T: Copy + std::fmt::Display>(nwmatrix: &Vec<Vec<Vec<(u8, u8, Direction)>>>, seq1: &Vec<T>, seq2: &Vec<T>) {
    let (fmt1, fmt2) = (seq1.iter().map(|x| format!("{}", x)).collect::<Vec<String>>(), seq2.iter().map(|x| format!("{}", x)).collect::<Vec<String>>());
    let max_len = fmt1.iter().chain(fmt2.iter()).map(|s| s.len()).max().unwrap();
    let seq2_fmt = pad_to_length(seq2.iter().map(|x| format!("{}", x)).collect(), max_len);
    print!("{}", pad_one_to_length("", 2*max_len + 2));
    println!("{}", seq2_fmt.join(" "));

    for (i, row) in nwmatrix.iter().enumerate() {
        let label = {
            if i == 0 { "".to_string() } else { format!("{}", seq1[i-1]) }
        };
        print!("{} ", pad_one_to_length(label, max_len));
        for cell in row.iter() {
            let (_, _, dirn) = best_candidate(cell);
            for _ in 0..(max_len/2) {
                print!(" ");
            }
            print!("{}", dirn);
            for _ in 0..(max_len/2) + max_len % 2 {
                print!(" ");
            }
        }
        println!();
    }
}

fn alignment_from_nwmatrix<T: Copy + std::fmt::Display>(seq1: &Vec<T>, seq2: &Vec<T>, nwmatrix: Vec<Vec<Vec<(u8, u8, Direction)>>>) -> Vec<(Option<T>, Option<T>)> {
    // build up the alignment in reverse order
    let mut aligned = Vec::new();
    let mut i = nwmatrix.len()-1;
    let mut j = nwmatrix[0].len()-1;
    loop {
        let candidates = &nwmatrix[i][j];
        if i == 0 && j == 0 {
            break;
        }
        let (_, _, dirn) = best_candidate(candidates);
        match dirn {
            Direction::Diag => {
                aligned.push((Some(seq1[i-1]), Some(seq2[j-1])));
                i -= 1;
                j -= 1;
            },
            Direction::Vert => {
                aligned.push((Some(seq1[i-1]), None));
                i -= 1;
            },
            Direction::Horz => {
                aligned.push((None, Some(seq2[j-1])));
                j -= 1;
            },
        }
    }

    print_dirn_matrix(&nwmatrix, seq1, seq2);

    aligned.reverse();
    aligned
}

fn pad_one_to_length<T: std::fmt::Display>(s: T, length: usize) -> String {
    let mut s_padded = format!("{}", s);
    while s_padded.len() < length {
        s_padded.push(' ');
    }
    s_padded
}

fn pad_to_length<T: std::fmt::Display>(seq: Vec<T>, length: usize) -> Vec<String> {
    seq.into_iter().map(|s| {pad_one_to_length(s, length)}).collect()
}

fn display_aligned<T: std::fmt::Display>(alignment: Vec<(Option<T>, Option<T>)>) -> String {
    fn format_one<T: std::fmt::Display>(seq: Vec<Option<T>>) -> Vec<String> {
        seq.into_iter().map(|x| match x {
            Some(x) => format!("{}", x),
            None => " ".to_string(),
        }).collect::<Vec<String>>()
    }

    // print the first sequence on top and the second sequence on the bottom, turning Nones into blank spaces
    let (first, second): (Vec<Option<T>>, Vec<Option<T>>) = alignment.into_iter().unzip();
    let (fmt1, fmt2) = (format_one(first), format_one(second));
    // pad out the elements to all be the same length
    let max_len = fmt1.iter().chain(fmt2.iter()).map(|s| s.len()).max().unwrap();
    let (fmt1, fmt2) = (pad_to_length::<String>(fmt1, max_len), pad_to_length::<String>(fmt2, max_len));

    format!("{}\n{}", fmt1.join(", "), fmt2.join(", "))
}

run_n_times!{100,
#[test]
fn alignment_multiple_insertions() {
    // get a random sequence of 10 integers (we don't need to use real chords; the alignment function can use anything that implements PartialEq)
    let mut rng = rand::thread_rng();
    let mut seq = Vec::new();
    const UNUSED_ELEM: usize = 9;
    const SEQ_LEN: usize = 10;
    for _ in 0..SEQ_LEN {
        seq.push(rng.gen_range(0..UNUSED_ELEM));
    }
    // now make a corrupted copy of seq
    let mut corrupted_seq = seq.clone();
    // change a few (maybe 0) elements
    for _ in 0..rng.gen_range(0..3) {
        let idx = rng.gen_range(0..corrupted_seq.len());
        corrupted_seq[idx] = rng.gen_range(0..UNUSED_ELEM);
    }

    // delete a few (maybe 0) elements
    for _ in 0..rng.gen_range(0..3) {
        let idx = rng.gen_range(0..corrupted_seq.len());
        corrupted_seq.remove(idx);
    }

    // insert a new element at a random position
    let insert_idx = rng.gen_range(0..corrupted_seq.len());
    corrupted_seq.insert(insert_idx, UNUSED_ELEM);

    println!("original sequence: {:?}", seq);
    println!("corrupted sequence: {:?}", corrupted_seq);
    
    let (original_correct, original_incorrect, original_nwmatrix) = align(&seq, &corrupted_seq);

    println!("original aligned sequence:\n{}", display_aligned(alignment_from_nwmatrix(&seq, &corrupted_seq, original_nwmatrix)));
    println!("score: {}", original_correct as f64 / (original_correct + original_incorrect) as f64);

    // now insert a few more of the same element at the same position
    let n_insertions = rng.gen_range(1..4);
    for _ in 0..n_insertions {
        corrupted_seq.insert(insert_idx, UNUSED_ELEM);
    }

    let (new_correct, new_incorrect, new_nwmatrix) = align(&seq, &corrupted_seq);

    println!("new aligned sequence:\n{}", display_aligned(alignment_from_nwmatrix(&seq, &corrupted_seq, new_nwmatrix)));
    println!("score: {}", new_correct as f64 / (new_correct + new_incorrect) as f64);

    // repeated insertions should not affect alignment quality
    assert!(new_correct == original_correct);
    assert!(new_incorrect == original_incorrect);
}
}
