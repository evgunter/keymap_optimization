use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct TrainingArgs {
    /// Random seed for reproducibility (affects weight initialization and train/test split)
    #[arg(short, long, default_value_t = 42)]
    pub seed: u64,

    /// Number of training epochs
    #[arg(short = 'e', long, default_value_t = 2001)]
    pub epochs: usize,
}
