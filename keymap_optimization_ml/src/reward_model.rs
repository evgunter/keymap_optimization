use tch::nn::{Module, Sequential};
use tch::{nn, Tensor};
use itertools::multiunzip;
use tuple::Map;

// we learn a pair of embeddings: one for accuracy, one for time--such that a function of the embeddings
// of two chords represents the predicted time and accuracy for alternation between them

// the input is a binary vector representing the keys pressed in the chord; so, its dimension is the number of keys
// these are the dimensions of the embeddings
const HIDDEN_DIM_SPEED: i64 = 8;
const HIDDEN_DIM_ACCURACY: i64 = 8;
const HIDDEN_DIM_POSSIBLE: i64 = 8;
const HIDDEN_NUM_LAYERS: i64 = 1;
const HIDDEN_DIM_SPEED_COMBINED: i64 = 4;
const HIDDEN_DIM_ACCURACY_COMBINED: i64 = 4;
const HIDDEN_SPEED_COMBINED_NUM_LAYERS: i64 = 0;
const HIDDEN_ACCURACY_COMBINED_NUM_LAYERS: i64 = 0;
const NUM_ENSEMBLE: usize = 10;

fn seq_in_mid_out(vs: &nn::Path, in_dim: i64, mid_dim: i64, out_dim: i64, n_mid_layers: i64) -> Sequential {
    // create a sequential neural network with dimensions:
    // in_dim -> mid_dim -> mid_dim -> ... -> mid_dim -> out_dim
    //           \__________________________________/
    //                    n_mid_layers times
    // where each -> includes a ReLU (note that there is no ReLU applied to the output)
    // in particular, it has a minimum of 2 layers; the input and output layers are always distinct
    let mut net = nn::seq()
                     .add(nn::linear(vs, in_dim, mid_dim, Default::default()))
                     .add_fn(|xs| xs.relu());
    for _ in 0..n_mid_layers {
        net = net.add(nn::linear(vs, mid_dim, mid_dim, Default::default()))
                 .add_fn(|xs| xs.relu());
    }
    net.add(nn::linear(vs, mid_dim, out_dim, Default::default()))
}

fn embed<const N: usize>(vs: &nn::Path, hidden_dim: i64) -> Sequential {
    seq_in_mid_out(vs, N as i64, hidden_dim, hidden_dim, HIDDEN_NUM_LAYERS - 1)
}

pub trait RewardEmbedding: std::fmt::Debug + std::marker::Send + Sized {
    fn new(vs: &nn::Path) -> Self;

    fn tt_to_flat(tt: (Tensor, Tensor, Tensor)) -> Tensor {
        Tensor::cat(&[&tt.0, &tt.1, &tt.2], 1)
    }

    fn flat_to_tt(flat: Tensor) -> (Tensor, Tensor, Tensor) {
        let split = flat.split_with_sizes(&[HIDDEN_DIM_SPEED, HIDDEN_DIM_ACCURACY, 1], 1);
        (split[0].shallow_clone(), split[1].shallow_clone(), split[2].shallow_clone())
    }

    fn embed_chords(&self, chords: &Tensor) -> (Tensor, Tensor, Tensor);

    fn forward(&self, xs: &Tensor) -> Tensor {
        Self::tt_to_flat(self.embed_chords(xs))
    }
}

#[derive(Debug)]
pub struct RewardEmbeddingBase<const N: usize> {
    speed: Sequential,
    accuracy: Sequential,
    is_possible: Sequential,
}

impl<const N: usize> RewardEmbedding for RewardEmbeddingBase<N> {
    fn new(vs: &nn::Path) -> Self {
        Self {
            speed: embed::<N>(&vs.sub("speed"), HIDDEN_DIM_SPEED),
            accuracy: embed::<N>(&vs.sub("accuracy"), HIDDEN_DIM_ACCURACY),
            is_possible: embed::<N>(&vs.sub("is_possible"), HIDDEN_DIM_POSSIBLE).add(nn::linear(vs, HIDDEN_DIM_POSSIBLE, 1, Default::default())).add_fn(|xs| xs.sigmoid()),
        }
    }

    fn embed_chords(&self, chords: &Tensor) -> (Tensor, Tensor, Tensor) {
        // the output tensors may not be the same shape
        let speed = self.speed.forward(chords);
        let accuracy = self.accuracy.forward(chords);
        let is_possible = self.is_possible.forward(chords);
        (speed, accuracy, is_possible)
    }
}

#[derive(Debug)]
pub struct RewardModel<const N: usize, E: RewardEmbedding> {
    pub chord_embedding: E,
    pub speed_combiner: Sequential,
    pub accuracy_combiner: Sequential,
}

pub struct Dataset {
    pub train_input: Tensor,
    pub train_target: Tensor,
    pub test_input: Tensor,
    pub test_target: Tensor,
}

impl<const N: usize, E: RewardEmbedding> Module for RewardModel<N, E> {
    fn forward(&self, xs: &Tensor) -> Tensor {
        let chords = xs.split_with_sizes(&[N as i64, N as i64], 1);
        // chords should consist of two entries
        let (chord_1, chord_2) = (&chords[0], &chords[1]);

        let ((emb_1_s, emb_1_a, ip_1), (emb_2_s, emb_2_a, ip_2)) = (self.chord_embedding.embed_chords(&chord_1), self.chord_embedding.embed_chords(&chord_2));
        let speed = self.speed_combiner.forward(&Tensor::cat(&[&emb_1_s, &emb_2_s], 1)).squeeze();
        let accuracy = self.accuracy_combiner.forward(&Tensor::cat(&[&emb_1_a, &emb_2_a], 1)).squeeze();

        // whether the combination is possible is entirely dependent on whether its constituent chords are possible
        let dim_sum = [-1i64];  // the first dimension is the batch size; so, to take the product of all the probabilities individually, we use sum_dim_intlist
        let is_possible = (ip_1 * ip_2).sum_dim_intlist(&dim_sum[..], false, tch::Kind::Float);

        Tensor::stack(&[speed, accuracy, is_possible], 1)
    }
}

impl<const N: usize, E: RewardEmbedding> RewardModel<N, E> {
    pub fn new(vs: &nn::Path) -> Self {
        Self {
            chord_embedding: E::new(&vs.sub("chord_embedding")),
            speed_combiner: seq_in_mid_out(&vs.sub("speed_combiner"), 2*HIDDEN_DIM_SPEED, HIDDEN_DIM_SPEED_COMBINED, 1, HIDDEN_SPEED_COMBINED_NUM_LAYERS).add_fn(|xs| xs.exp()),  // scale to 0, infinity with exp
            accuracy_combiner: seq_in_mid_out(&vs.sub("accuracy_combiner"), 2*HIDDEN_DIM_ACCURACY, HIDDEN_DIM_ACCURACY_COMBINED, 1, HIDDEN_ACCURACY_COMBINED_NUM_LAYERS).add_fn(|xs| xs.sigmoid()),  // scale to 0, 1 with sigmoid
        }
    }
}

pub fn loss<const N: usize, E: RewardEmbedding>(model: &RewardModel<N, E>, input: &Tensor, target: &Tensor) -> Tensor {
    // the output is part numerical (speed, accuracy) and part categorical (is_possible).
    // so, the loss is the mean squared error of the numerical part + (a multiple of) the binary cross entropy of the categorical part
    const XE_WEIGHT: f64 = 100.0;
    let output = &model.forward(input);

    fn split_numeric_categorical(tn: &Tensor) -> (Tensor, Tensor) {
        match tn.split_with_sizes(&[2, 1], 1).as_slice() {
            [numeric, categorical] => (numeric.shallow_clone(), categorical.shallow_clone()),
            _ => panic!("tensor has the wrong number of dimensions"),
        }
    }
    let (numeric_out, categorical_out) = split_numeric_categorical(output);
    let (numeric_target, categorical_target) = split_numeric_categorical(target);

    let mse_part = numeric_out.mse_loss(&numeric_target, tch::Reduction::Mean);
    let bce_part = categorical_out.binary_cross_entropy_with_logits::<Tensor>(&categorical_target, None, None, tch::Reduction::Mean);
    mse_part + XE_WEIGHT * bce_part
}

#[derive(Debug)]
pub struct Ensemble<M: Module> {
    models: Vec<Box<M>>,
}

impl<M: Module> Ensemble<M> {
    pub fn from_submodels(models: Vec<Box<M>>) -> Self {
        Self { models }
    }
}

impl<const N: usize, E: RewardEmbedding> RewardEmbedding for Ensemble<RewardModel<N, E>> {
    fn new(vs: &nn::Path) -> Self {
        Self { models: (0..NUM_ENSEMBLE).map(|_| Box::new(RewardModel::<N, E>::new(vs))).collect() }
    }

    fn embed_chords(&self, chords: &Tensor) -> (Tensor, Tensor, Tensor) {
        let embeddings = self.models.iter().map(|m| m.chord_embedding.embed_chords(chords)).collect::<Vec<(Tensor, Tensor, Tensor)>>();
        multiunzip::<(Vec<Tensor>, Vec<Tensor>, Vec<Tensor>), Vec<(Tensor, Tensor, Tensor)>>(embeddings).map(|ts| Tensor::stack(&ts, 0).mean_dim(0, false, tch::Kind::Float))
    }
}

impl<const N: usize, E: RewardEmbedding> RewardModel<N, Ensemble<RewardModel<N, E>>> {

}
