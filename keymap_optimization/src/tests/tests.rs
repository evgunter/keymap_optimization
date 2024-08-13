#![cfg(test)]

use crate::keyboard_config::Chord;
use crate::twiddler::{TwiddlerLayout, TwiddlerKey};
use crate::chord_preferences::logic::{TrialResults, TrialData, random_chord, ErrCode};
use rand::Rng;
use strum::{EnumCount, VariantArray};

fn make_demo_trial<R: Rng> (rng: &mut R, threshold: f64, impossible_threshold: f64) -> TrialData<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> {
    let n_repetitions_per_trial = rng.gen_range(1..10);  // This will actually be fixed in practice, but doesn't hurt to vary it here
    // some of the time, get a random duration uniformly sampled between 0.0 and 100.0; some of the time, use ErrCode::Impossible
    let time_elapsed: Result<f64, ErrCode> = {
        if rng.gen::<f64>() < impossible_threshold {
            Err(ErrCode::Impossible)
        } else {
            Ok(100.0 * rng.gen::<f64>())
        }
    };
    TrialData {
        chord_pair: [random_chord(rng, threshold), random_chord(rng, threshold)],
        n_repetitions: n_repetitions_per_trial,
        time: time_elapsed,
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

#[test]
fn serialization_round_trip_success() {
    // Write some demo results to file, then load them from file and verify that they are identical.
    const RESULTS_PATH: &str = "/tmp";
    // note that it's important to have the files have different names--tests are run concurrently!
    let results_path = format!("{}/chord_preferences_results_success_{}.json", RESULTS_PATH, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
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

#[test]
fn serialization_round_trip_chord_edited() {
    // Write some demo results to file, then load them from file and verify that they are identical.
    const RESULTS_PATH: &str = "/tmp";
    // note that it's important to have the files have different names--tests are run concurrently!
    let results_path = format!("{}/chord_preferences_results_failure_{}.json", RESULTS_PATH, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let mut demo_results = make_demo_data_default();

    match demo_results.save(&results_path) {
        Ok(_) => println!("Results saved successfully."),
        Err(e) => return assert!(false, "Error saving results: {}", e)
    }

    // now edit results to do one of the following:
    // (a) add a new trial at a random position;
    // (b) remove a random trial;
    // (c) flip a random key in a random chord;
    // (d) change n_repetitions in a random trial
    // (e) change time in a random trial
    let mut rng = rand::thread_rng();
    let val = rng.gen::<f64>();
    let idx = if demo_results.data.is_empty() { 0 } else { rng.gen_range(0..demo_results.data.len()) };
    // if there are no trials, we can only do (a)
    if val < 0.2 || demo_results.data.is_empty() {
        // (a) add a new trial at a random position
        demo_results.data.insert(idx, make_demo_trial(&mut rng, 0.8, 0.2));
    } else if val < 0.4 {
        // (b) remove a random trial
        demo_results.data.remove(idx);
    } else if val < 0.6 {
        // (c) flip a random key in a random chord
        let chord_idx = rng.gen_range(0..2);
        let key_idx = rng.gen_range(0..TwiddlerKey::COUNT);
        let chord_keys = &mut demo_results.data[idx].chord_pair[chord_idx].get_raw_keys();
        chord_keys[key_idx] = !chord_keys[key_idx];
    } else if val < 0.8 {
        // (d) change n_repetitions in a random trial
        demo_results.data[idx].n_repetitions += 1;
    } else {
        // (e) change time in a random trial
        demo_results.data[idx].time = match demo_results.data[idx].time {
            Ok(old_time) => {
                if rng.gen::<f64>() < 0.5 { Err(ErrCode::Impossible) } else { Ok(old_time + 1.0) }
            }
            Err(ErrCode::Impossible) => Ok(100.0 * rng.gen::<f64>()),
        }
    }

    // now load the results and verify that they are NOT the same as our edited results
    let loaded_results = match TrialResults::load(&results_path) {
        Ok(loaded_results) => loaded_results,
        Err(e) => return assert!(false, "Error loading results: {}", e)
    };

    assert!(loaded_results != demo_results)
}


#[test]
fn chord_display()
where {
    // check that displaying some chords, including an empty chord anda full chord, does not panic
    let empty_chord: Chord<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> = Chord::new();
    let mut full_chord: Chord<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> = Chord::new();
    for key in TwiddlerKey::VARIANTS.iter() {
        full_chord.add_key(*key);
    }
    let regular_chord: Chord<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> = random_chord(&mut rand::thread_rng(), 0.8);

    println!("Empty chord: {}", empty_chord);
    println!("Full chord: {}", full_chord);
    println!("Regular chord: {}", regular_chord);
}