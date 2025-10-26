use keymap_optimization::twiddler::{TwiddlerKey as K, TwiddlerLayout as L};
use keymap_optimization::local_env::DATA_PATH;
use strum::EnumCount;
use clap::Parser;

use keymap_optimization_ml::train::run;

#[cfg(not(any(feature = "model-single", feature = "model-ensemble")))]
compile_error!("a model type is required for training");

#[cfg(all(feature = "model-single", feature = "model-ensemble"))]
compile_error!("exactly one model type is required for training");

#[cfg(feature = "model-single")]
type E = keymap_optimization_ml::reward_model::RewardEmbeddingBase<{ K::COUNT }>;

#[cfg(feature = "model-ensemble")]
type E = keymap_optimization_ml::reward_model::Ensemble<keymap_optimization_ml::reward_model::RewardModel<{ K::COUNT }, keymap_optimization_ml::reward_model::RewardEmbeddingBase<{ K::COUNT }>>>;

#[derive(Parser, Debug)]
#[command(name = "train_twiddler")]
#[command(about = "Train Twiddler reward model", long_about = None)]
struct Args {
    /// Random seed for reproducibility (affects weight initialization and train/test split)
    #[arg(short, long, default_value_t = 42)]
    seed: u64,

    /// Number of training epochs
    #[arg(short = 'e', long, default_value_t = 2001)]
    epochs: usize,
}

fn main() {
    let args = Args::parse();
    run::<K, { K::COUNT }, L, E>(DATA_PATH, args.epochs, args.seed);
}
