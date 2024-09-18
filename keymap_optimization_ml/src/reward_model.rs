use tch::nn::{Module, Sequential};
use tch::{nn, Tensor};

// we learn a pair of embeddings: one for accuracy, one for time--such that the inner product of
// the embedding of two chords represents the predicted time and accuracy for alternation between them
// TODO: there will need to be some kind of scaling after the inner products to convert these into
// actual time and accuracy scores. e.g. accuracy is between 0 and 1 (so we could use a sigmoid);
// time could be the inner product + a learned bias (since even alternating between the same chord
// takes more than 0 time).

// the input is a binary vector representing the keys pressed in the chord; so, its dimension is the number of keys
// these are the dimensions of the embeddings
const HIDDEN_DIM_SPEED: i64 = 3;
const HIDDEN_DIM_ACCURACY: i64 = 3;
const HIDDEN_DIM_POSSIBLE: i64 = 3;

fn embed<const N: usize>(vs: &nn::Path, hidden_dim: i64) -> Sequential {
    // TODO: figure out how many layers to have
    // currently, 1 input layer and 1 hidden layer
    nn::seq()
        .add(nn::linear(vs, N as i64, hidden_dim, Default::default()))
        .add_fn(|xs| xs.relu())
        .add(nn::linear(vs, hidden_dim, hidden_dim, Default::default()))
        .add_fn(|xs| xs.relu())
}

pub struct RewardEmbedding {
    speed: Sequential,
    accuracy: Sequential,
    is_possible: Sequential,
}

impl RewardEmbedding {
    pub fn new<const N: usize>(vs: &nn::Path) -> Self {
        Self {
            speed: embed::<N>(&vs.sub("speed"), HIDDEN_DIM_SPEED),
            accuracy: embed::<N>(&vs.sub("accuracy"), HIDDEN_DIM_ACCURACY),
            is_possible: embed::<N>(&vs.sub("is_possible"), HIDDEN_DIM_POSSIBLE).add(nn::linear(vs, HIDDEN_DIM_POSSIBLE, 1, Default::default())).add_fn(|xs| xs.sigmoid()),
        }
    }

    pub fn forward(&self, xs: &Tensor) -> (Tensor, Tensor, Tensor) {
        // the output tensors may not be the same chape, so they can't be combined into a single tensor
        let speed = self.speed.forward(xs);
        let accuracy = self.accuracy.forward(xs);
        let is_possible = self.is_possible.forward(xs);
        (speed, accuracy, is_possible)
    }
}

pub struct RewardModel {
    pub chord_embedding: RewardEmbedding,
    pub speed_bias: Tensor,
}

pub struct Dataset {
    pub train_input: Tensor,
    pub train_target: Tensor,
    pub test_input: Tensor,
    pub test_target: Tensor,
}

impl RewardModel {
    pub fn new<const N: usize>(vs: &nn::Path) -> Self {
        Self {
            chord_embedding: RewardEmbedding::new::<N>(&vs.sub("chord_embedding")),
            speed_bias: vs.var("speed_bias", &[1], tch::nn::Init::Const(0.0)),
        }
    }

    fn forward<const N: usize>(&self, xs: &Tensor) -> Tensor {
        let chords = xs.split_with_sizes(&[N as i64, N as i64], 1);
        // chords should consist of two entries
        let (chord_1, chord_2) = (&chords[0], &chords[1]);

        let ((emb_1_s, emb_1_a, ip_1), (emb_2_s, emb_2_a, ip_2)) = (self.chord_embedding.forward(&chord_1), self.chord_embedding.forward(&chord_2));
        // the first dimension is the batch size; so, to take the dot product of all the embeddings individually, we use sum_dim_intlist
        let dim_sum = [-1i64];
        let inner_product_speed = (emb_1_s * emb_2_s).sum_dim_intlist(&dim_sum[..], false, tch::Kind::Float);
        let inner_product_accuracy = (emb_1_a * emb_2_a).sum_dim_intlist(&dim_sum[..], false, tch::Kind::Float);

        // whether the combination is possible is entirely dependent on whether its constituent chords are possible
        let is_possible = (ip_1 * ip_2).sum_dim_intlist(&dim_sum[..], false, tch::Kind::Float);

        // we apply a sigmoid to the accuracy to scale it to [0, 1]
        let accuracy: Tensor = inner_product_accuracy.sigmoid();
        // the speed will be exp(1 - inner_product_speed) - learned_bias to scale it to (0, infinity), with learned_bias representing the value for alternation between the same chord
        let speed: Tensor = (Tensor::ones_like(&inner_product_speed) - &inner_product_speed + &self.speed_bias).exp();
        Tensor::stack(&[speed, accuracy, is_possible], 1)
    }
}

pub fn loss<const N: usize>(model: &RewardModel, input: &Tensor, target: &Tensor) -> Tensor {
    // the output is part numerical (speed, accuracy) and part categorical (is_possible).
    // so, the loss is the mean squared error of the numerical part + (a multiple of) the binary cross entropy of the categorical part
    const XE_WEIGHT: f64 = 10.0;
    let output = &model.forward::<N>(input);

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
