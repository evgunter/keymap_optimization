use tch::nn::{Module, OptimizerConfig, Sequential, Linear};
use tch::{nn, Tensor};
use keymap_optimization::keyboard_config::{Chord, Layout, Key};
use keymap_optimization::chord_preferences::TrialResults;
use keymap_optimization::local_env::RESULTS_PATH;
use keymap_optimization::chord_preferences::gather_chords::{ErrCode, accuracy_from_chord_pair};
use rand::distributions::Standard;
use rand::prelude::Distribution;

// we learn a pair of embeddings: one for accuracy, one for time--such that the inner product of
// the embedding of two chords represents the predicted time and accuracy for alternation between them
// TODO: there will need to be some kind of scaling after the inner products to convert these into
// actual time and accuracy scores. e.g. accuracy is between 0 and 1 (so we could use a sigmoid);
// time could be the inner product + a learned bias (since even alternating between the same chord
// takes more than 0 time).

// the input is a binary vector representing the keys pressed in the chord; so, its dimension is the number of keys
const HIDDEN_DIM_SPEED: i64 = 64;
const HIDDEN_DIM_ACCURACY: i64 = 64;

fn embed<const N: usize>(vs: &nn::Path, hidden_dim: i64) -> Sequential {
    // TODO: figure out how many layers to have
    // currently, 1 input layer and 2 hidden layers
    nn::seq()
        .add(nn::linear(vs, N as i64, hidden_dim, Default::default()))
        .add_fn(|xs| xs.relu())
        .add(nn::linear(vs, hidden_dim, hidden_dim, Default::default()))
        .add_fn(|xs| xs.relu())
        .add(nn::linear(vs, hidden_dim, hidden_dim, Default::default()))
        .add_fn(|xs| xs.relu())
}

struct RewardEmbedding {
    speed: Sequential,
    accuracy: Sequential,
}

impl RewardEmbedding {
    fn new<const N: usize>(vs: &nn::Path) -> Self {
        Self {
            speed: embed::<N>(&vs.sub("speed"), HIDDEN_DIM_SPEED),
            accuracy: embed::<N>(&vs.sub("accuracy"), HIDDEN_DIM_ACCURACY),
        }
    }

    fn forward(&self, xs: &Tensor) -> Tensor {
        let speed = self.speed.forward(xs);
        let accuracy = self.accuracy.forward(xs);
        Tensor::cat(&[speed, accuracy], 1)
    }
}

struct RewardModel {
    chord_1_embedding: RewardEmbedding,
    chord_2_embedding: RewardEmbedding,
    final_layer: Linear,
}

struct Dataset {
    train_input: Tensor,
    train_target: Tensor,
}

impl RewardModel {
    fn new<const N: usize>(vs: &nn::Path) -> Self {
        Self {
            chord_1_embedding: RewardEmbedding::new::<N>(&vs.sub("chord_1")),
            chord_2_embedding: RewardEmbedding::new::<N>(&vs.sub("chord_2")),
            final_layer: nn::linear(vs.sub("final"), 2 * (HIDDEN_DIM_SPEED + HIDDEN_DIM_ACCURACY), 2, Default::default()),
        }
    }

    fn forward<const N: usize>(&self, xs: &Tensor) -> Tensor {
        let chords = xs.split_with_sizes(&[N as i64, N as i64], 1);
        // chords should consist of two entries
        let (chord_1, chord_2) = (&chords[0], &chords[1]);

        let (emb_1, emb_2) = (self.chord_1_embedding.forward(&chord_1), self.chord_2_embedding.forward(&chord_2));
        let emb_pair = Tensor::cat(&[emb_1, emb_2], 1);
        emb_pair.apply(&self.final_layer)
    }
}

fn load_data<K: Key, const N: usize, L: Layout<K, N>>() -> Result<TrialResults<K, N, L>, Box<dyn std::error::Error>> where Standard: Distribution<K> {
    // load the data from all the files chord_preferences_results*.json in RESULTS_PATH
    let files: Vec<std::fs::DirEntry> = std::fs::read_dir(RESULTS_PATH)?
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

fn chord_to_tensor<K: Key, const N: usize, L: Layout<K, N>>(chord: &Chord<K, N, L>) -> Tensor where Standard: Distribution<K> {
    Tensor::f_from_slice(&chord.to_vector().into_iter().map(|c| if c { 1.0 } else { 0.0 }).collect::<Vec<f32>>()).unwrap()
}

fn get_formatted_data<K: Key, const N: usize, L: Layout<K, N>>() -> Result<Dataset, Box<dyn std::error::Error>> where Standard: Distribution<K> {
    let results: TrialResults<K, N, L> = load_data::<K, N, L>()?;
    let paired: Vec<([Chord<K, N, L>; 2], [f32; 2])> = results.data.into_iter().map(|trial| {
        match trial.performance {
            Err(ErrCode::Impossible) => (trial.chord_pair, [0.0, 0.0]),
            Ok(perf) => {
                let accuracy = accuracy_from_chord_pair(&perf.input, &trial.chord_pair) as f32;
                (trial.chord_pair, [perf.time as f32, accuracy])
            },
        }
    }).collect();
    let (input, target): (Vec<Tensor>, Vec<Tensor>) = paired.into_iter()
                                                            .map(|(chord_pair, perf)| { Ok((Tensor::concat(&chord_pair.map(|c| chord_to_tensor(&c)), 0),
                                                                                         Tensor::f_from_slice(&perf)?)) })
                                                            .collect::<Result<Vec<(Tensor, Tensor)>, tch::TchError>>()?
                                                            .into_iter()
                                                            .unzip();
    Ok(Dataset {
        train_input: Tensor::stack(&input, 0),
        train_target: Tensor::stack(&target, 0),
    })
}

pub fn train<K: Key, const N: usize, L: Layout<K, N>>() -> Result<(), Box<dyn std::error::Error>> where Standard: Distribution<K> {
    let vs = nn::VarStore::new(tch::Device::Cpu);
    let model = RewardModel::new::<N>(&vs.root());
    let mut opt = nn::Adam::default().build(&vs, 1e-3)?;
    let data = get_formatted_data::<K, N, L>()?;
    for epoch in 1..1000 {
        // we can process all the data at once since it's quite small
        let loss = model.forward::<N>(&data.train_input).mse_loss(&data.train_target, tch::Reduction::Mean);
        opt.backward_step(&loss);
        if epoch % 100 == 0 {
            println!("epoch: {:4} loss: {}", epoch, loss.double_value(&[]));
        }
    }
    Ok(())
}

pub fn run<K: Key, const N: usize, L: Layout<K, N>>() where Standard: Distribution<K> {
    match train::<K, N, L>() {
        Ok(()) => (),
        Err(e) => {
            eprintln!("Error during training: {}", e);
            return;
        }
    };
}