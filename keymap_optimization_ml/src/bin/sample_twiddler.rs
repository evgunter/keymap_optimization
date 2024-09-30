use keymap_optimization::twiddler::{TwiddlerKey as K, TwiddlerLayout as L, TwiddlerChordTrialUtils as C};
use strum::EnumCount;
use rand::rngs::ThreadRng as R;

use keymap_optimization::chord_preferences::data_collection_keymap_gen::run;

// check that the feature settings are valid

#[cfg(all(not(feature = "sampler-exponential"), not(feature = "sampler-possible"), not(feature = "sampler-uncertain")))]
compile_error!("at least one of the features 'sampler-exponential', 'sampler-possible', or 'sampler-uncertain' must be enabled");

#[cfg(all(feature = "sampler-exponential", any(feature = "sampler-possible", feature = "sampler-uncertain")))]
compile_error!("only one of the features 'sampler-exponential', 'sampler-possible', or 'sampler-uncertain' can be enabled");

#[cfg(all(feature = "sampler-possible", any(feature = "sampler-exponential", feature = "sampler-uncertain")))]
compile_error!("only one of the features 'sampler-exponential', 'sampler-possible', or 'sampler-uncertain' can be enabled");

#[cfg(all(feature = "sampler-uncertain", any(feature = "sampler-exponential", feature = "sampler-possible")))]
compile_error!("only one of the features 'sampler-exponential', 'sampler-possible', or 'sampler-uncertain' can be enabled");

#[cfg(all(feature = "sampler-exponential", any(feature = "model-single", feature = "model-ensemble")))]
compile_error!("exponential sampler does not use a model");

#[cfg(all(any(feature = "sampler-possible", feature = "sampler-uncertain"), not(feature = "model-single"), not(feature = "model-ensemble")))]
compile_error!("possible and uncertain samplers require a model");

// define the types based on the feature settings

#[cfg(feature = "sampler-exponential")]
type S = keymap_optimization::twiddler::TwiddlerExponentialSampler<R>;
#[cfg(feature = "sampler-exponential")]
type E = ();

#[cfg(feature = "sampler-possible")]
type S = keymap_optimization_ml::chord_samplers::PossibleChordSampler<K, { K::COUNT }, L, R>;

#[cfg(feature = "sampler-uncertain")]
type S = keymap_optimization_ml::chord_samplers::MostUncertainPossibilityChordSampler<K, { K::COUNT }, L, R>;

#[cfg(feature = "model-single")]
type E = keymap_optimization_ml::reward_model::RewardEmbeddingBase<{ K::COUNT }>;

#[cfg(feature = "model-ensemble")]
type E = keymap_optimization_ml::reward_model::Ensemble<keymap_optimization_ml::reward_model::RewardModel<{ K::COUNT }, keymap_optimization_ml::reward_model::RewardEmbeddingBase<{ K::COUNT }>>>;

fn main() {
    #[cfg(feature = "sampler-exponential")]
    let initialization_info = ();

    #[cfg(any(feature = "sampler-possible", feature = "sampler-uncertain"))]
    let initialization_info = match keymap_optimization_ml::train::train::<K, { K::COUNT }, L, E>(keymap_optimization::local_env::DATA_PATH, 2001) {
        Ok(model) => Box::new(model.chord_embedding),
        Err(e) => panic!("error training model: {}", e)
    };

    run::<K, { K::COUNT }, L, E, S, C>(&initialization_info);

}
