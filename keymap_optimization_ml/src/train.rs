use tch::nn::OptimizerConfig;
use tch::{nn, Tensor};
use keymap_optimization::keyboard_config::{Chord, Layout, Key};
use keymap_optimization::chord_preferences::TrialResults;
use keymap_optimization::chord_preferences::gather_chords::{ErrCode, accuracy_from_chord_pair};
use rand::prelude::SliceRandom;

use crate::reward_model::{RewardModel, Dataset, loss};

const TEST_FRAC: f64 = 0.1;

pub fn chord_to_tensor<K: Key, const N: usize, L: Layout<K, N>>(chord: &Chord<K, N, L>) -> Tensor {
    Tensor::f_from_slice(&chord.to_vector().into_iter().map(|c| if c { 1.0 } else { 0.0 }).collect::<Vec<f32>>()).unwrap()
}

fn load_data<K: Key, const N: usize, L: Layout<K, N>>(results_path: &str) -> Result<TrialResults<K, N, L>, Box<dyn std::error::Error>> {
    // load the data from all the files chord_preferences_results*.json in RESULTS_PATH
    println!("loading data from {}", results_path);
    let files: Vec<std::fs::DirEntry> = std::fs::read_dir(results_path)?
        .filter(|f|
            match f {
                Ok(f) => {
                    let filename = f.file_name();
                    let filename = filename.to_string_lossy();
                    filename.starts_with("chord_preferences_results") && filename.ends_with(".json")
                }
                Err(_) => false,
            })
        .collect::<Result<Vec<std::fs::DirEntry>, std::io::Error>>()?;
    let mut all_results = TrialResults::new();
    for file in files {
        let results: TrialResults<K, N, L> = serde_json::from_reader(std::fs::File::open(file.path())?)?;
        all_results.data.extend(results.data);
    }
    Ok(all_results)
}

fn get_formatted_data<K: Key, const N: usize, L: Layout<K, N>>(results_path: &str) -> Result<Dataset, Box<dyn std::error::Error>> {
    let results: TrialResults<K, N, L> = load_data::<K, N, L>(results_path)?;
    let paired: Vec<([Chord<K, N, L>; 2], [f32; 3])> = results.data.into_iter().map(|trial| {
        match trial.performance {
            Err(ErrCode::Impossible) => (trial.chord_pair, [0.0, 0.0, 1.0]),
            Ok(perf) => {
                let accuracy = accuracy_from_chord_pair(&perf.input, &trial.chord_pair) as f32;
                (trial.chord_pair, [perf.time as f32, accuracy, 0.0])
            },
        }
    }).collect();
    println!("loaded {} trials", paired.len());

    let (input, target): (Vec<Tensor>, Vec<Tensor>) = paired.into_iter()
                                                            .map(|(chord_pair, perf)| { Ok((Tensor::concat(&chord_pair.map(|c| chord_to_tensor(&c)), 0),
                                                                                         Tensor::f_from_slice(&perf)?)) })
                                                            .collect::<Result<Vec<(Tensor, Tensor)>, tch::TchError>>()?
                                                            .into_iter()
                                                            .unzip();

    // split into train and test divisions
    let tot_len = input.len();
    let num_test = (tot_len as f64 * TEST_FRAC).round() as usize;
    // choose num_train random indices
    let mut indices: Vec<usize> = (0..tot_len).collect();
    indices.shuffle(&mut rand::thread_rng());
    println!("split into {} training examples, {} test examples", tot_len - num_test, num_test);
    let mut train_indices = indices.split_off(num_test);
    train_indices.sort();
    train_indices.reverse();

    let mut train_input = Vec::new();
    let mut train_target = Vec::new();
    let mut test_input = Vec::new();
    let mut test_target = Vec::new();
    for (idx, (inp, tar)) in input.into_iter().zip(target.into_iter()).enumerate() {
        if train_indices.len() > 0 && train_indices[train_indices.len()-1] == idx {
            train_indices.pop();
            train_input.push(inp);
            train_target.push(tar);
        } else {
            test_input.push(inp);
            test_target.push(tar);
        }
    }

    Ok(Dataset { train_input: Tensor::stack(&train_input, 0), train_target: Tensor::stack(&train_target, 0),
                 test_input: Tensor::stack(&test_input, 0), test_target: Tensor::stack(&test_target, 0) })
}


pub fn train<K: Key, const N: usize, L: Layout<K, N>>(results_path: &str) -> Result<RewardModel, Box<dyn std::error::Error>> {
    let vs = nn::VarStore::new(tch::Device::Cpu);
    let model = RewardModel::new::<N>(&vs.root());
    let mut opt = nn::Adam::default().build(&vs, 1e-3)?;
    let data = get_formatted_data::<K, N, L>(results_path)?;
    for epoch in 0..1001 {
        // we can process all the data at once since it's quite small
        let train_loss = loss::<N>(&model, &data.train_input, &data.train_target);
        opt.backward_step(&train_loss);
        if epoch % 100 == 0 {
            let test_loss = loss::<N>(&model, &data.test_input, &data.test_target);
            println!("epoch: {:<5} train loss: {:<24}, test loss: {:<24}", epoch, (train_loss.double_value(&[])) as f32, (test_loss.double_value(&[])) as f32);
        }
    }
    Ok(model)
}

pub fn run<K: Key, const N: usize, L: Layout<K, N>>(results_path: &str) {
    match train::<K, N, L>(results_path) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error during training: {}", e);
            return;
        }
    };
}
