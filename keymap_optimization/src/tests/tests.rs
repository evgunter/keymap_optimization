#![cfg(test)]

use crate::keyboard_config::Chord;
use crate::twiddler::{TwiddlerLayout, TwiddlerKey};
use crate::chord_preferences::logic::{TrialResults, TrialData, random_chord, ErrCode};
use rand::Rng;
use strum::{EnumCount, VariantArray};

fn make_demo_trial<R: Rng> (rng: &mut R, threshold: f64, impossible_threshold: f64) -> TrialData<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout> {
    let n_repetitions_per_trial = rng.gen_range(1..10);  // this will actually be fixed in practice, but doesn't hurt to vary it here
    // sometimes get a random duration uniformly sampled between 0.0 and 100.0 and a random accuracy uniformly sampled between 0.0 and 1.0;
    // sometimes use ErrCode::Impossible
    let trial_performance: Result<(f64, f64), ErrCode> = {
        if rng.gen::<f64>() < impossible_threshold {
            Err(ErrCode::Impossible)
        } else {
            Ok((100.0 * rng.gen::<f64>(), rng.gen::<f64>()))
        }
    };
    TrialData {
        chord_pair: [random_chord(rng, threshold), random_chord(rng, threshold)],
        n_repetitions: n_repetitions_per_trial,
        performance: trial_performance,
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

    assert!(loaded_results != demo_results)
}

#[test]
fn serialization_round_trip_add_trial() {
    // check that adding a new trial at a random position does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        demo_results.data.insert(idx, make_demo_trial(rng, 0.8, 0.2));
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_add_trial", edit_fn);
}

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

#[test]
fn serialization_round_trip_toggle_performance_error() {
    // check that switching performance between an error and a result does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }
        demo_results.data[idx].performance = match demo_results.data[idx].performance {
            Ok(_) => Err(ErrCode::Impossible),
            Err(ErrCode::Impossible) => Ok((100.0 * rng.gen::<f64>(), rng.gen::<f64>())),
        };
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_toggle_performance_error", edit_fn);
}

#[test]
fn serialization_round_trip_change_time() {
    // check that changing time in a random trial does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }
        demo_results.data[idx].performance = match demo_results.data[idx].performance {
            Ok((old_time, old_accuracy)) => Ok((old_time + 1.0, old_accuracy)),
            Err(ErrCode::Impossible) => Ok((100.0 * rng.gen::<f64>(), rng.gen::<f64>())),
        };
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_change_time", edit_fn);
}

#[test]
fn serialization_round_trip_change_accuracy() {
    // check that changing accuracy in a random trial does cause the results to be detected as different
    fn edit_fn(idx: usize, demo_results: &mut TrialResults<TwiddlerKey, { TwiddlerKey::COUNT }, TwiddlerLayout>, rng: &mut rand::prelude::ThreadRng) -> Result<(), &'static str> {
        if demo_results.data.is_empty() {
            return Err("no trials");
        }
        demo_results.data[idx].performance = match demo_results.data[idx].performance {
            Ok((old_time, old_accuracy)) => Ok((old_time, if old_accuracy == 0.5 { 0.0 } else { 1.0 - old_accuracy })),
            Err(ErrCode::Impossible) => Ok((100.0 * rng.gen::<f64>(), rng.gen::<f64>())),
        };
        Ok(())
    }
    serialization_round_trip_chord_edited("serialization_round_trip_change_accuracy", edit_fn);
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