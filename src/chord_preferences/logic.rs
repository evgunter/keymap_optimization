use rand::Rng;
use rand::distributions::{Distribution, Standard};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use std::{array, vec};
use std::collections::HashMap;

use crate::keyboard_config::{Key, Chord, Layout};
use crate::local_env::RESULTS_PATH;

const N_REPETITIONS_PER_TRIAL: usize = 5;

pub const CHORD_KEY_SAMPLE_THRESHOLD: f64 = 0.8;

#[derive(PartialEq, Debug)]
#[derive(Serialize, Deserialize)]
pub enum ErrCode {
    Impossible,
}

#[derive(PartialEq, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(bound = "K: DeserializeOwned, L: DeserializeOwned")]
pub struct TrialData<K: Key, const N: usize, L: Layout<K, N>> where Standard: Distribution<K> {
    pub chord_pair: [Chord<K, N, L>; 2],
    pub n_repetitions: usize,
    pub performance: Result<(f64, f64), ErrCode>,  // the first element is the total time, the second is the accuracy
}

#[derive(PartialEq, Debug)]
#[derive(Serialize, Deserialize)]
#[serde(bound = "K: DeserializeOwned, L: DeserializeOwned")]
pub struct TrialResults<K: Key, const N: usize, L: Layout<K, N>> where Standard: Distribution<K> {
    pub data: Vec<TrialData<K, N, L>>,
}

impl<K: Key, const N: usize, L: Layout<K, N>> TrialResults<K, N, L> where Standard: Distribution<K> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
        }
    }

    pub fn push(&mut self, trial_data: TrialData<K, N, L>) {
        self.data.push(trial_data);
    }

    pub fn save(&self, filename: &str) -> std::io::Result<()> {
        let file = std::fs::File::create(filename)?;
        serde_json::to_writer(file, self)?;
        Ok(())
    }

    pub fn load(filename: &str) -> std::io::Result<Self> {
        let file = std::fs::File::open(filename)?;
        let results = serde_json::from_reader(file)?;
        Ok(results)
    }
}

pub fn random_chord<R: Rng, K: Key, const N: usize, L: Layout<K, N>>(rng: &mut R, threshold: f64) -> Chord<K, N, L> where Standard: Distribution<K> {
    // sample a random chord with a number of keys distributed almost exponentially with base 1/threshold
    // (not exactly exponential because we are sampling with replacement and we always sample at least one key)
    let mut chord = Chord::new();
    chord.add_key(rng.gen::<K>());  // ensure that the chord contains at least one key
    loop {
        let val: f64 = rng.gen::<f64>();
        if val < threshold {
            chord.add_key(rng.gen::<K>());
        } else {
            break;
        }
    }
    chord
}

fn get_expected_input<K: Key, const N: usize, L: Layout<K, N>>(chords: &[Chord<K, N, L>; 2]) -> [String; 2 * N_REPETITIONS_PER_TRIAL] where Standard: Distribution<K> {
    // get the expected output for each chord
    let component: [String; 2] = array::from_fn(|i| {
        K::VARIANTS
            .iter()
            .filter(|key| chords[i].contains(**key))
            .map(|key| format!("{}", key))
            .collect::<String>()
    });
    // then copy this expected output N_REPETITIONS_PER_TRIAL times
    array::from_fn(|i| component[i % 2].clone())
}

fn alignment_quality(seq_predicted: Vec<String>, seq_corrupted: Vec<String>) -> (u8, u8) {
    // returns the number of correct chords and the number of incorrect chords after alignment.
    // currently we treat the two sequences identically, using a dynamic programming algorithm
    // similar to needleman-wunch but optimizing for the fraction of the total chords that are correct.
    // however, it may be desirable to treat the sequences asymmetrically, since we know that one of them
    // is "correct". hence, it is recommended to always use the first argument of this function for the
    // "correct" sequence and the second for the "incorrect" sequence.

    // as in the typical needleman-wunch algorithm, we initialize a matrix                    P R E D I C T E D
    // with shape (seq_predicted.len() + 1) x (seq_corrupted.len() + 1).                   |_|_|_|_|_|_|_|_|_|_|
    // the alignment will be represented as a path through this matrix,                  C |||_|_|_|_|_|_|_|_|_|
    // starting from the top left (before the start of both sequences) and               O |_|\|_|_|_|_|_|_|_|_|
    // ending in the top right (after the end of both sequences).                        R |_|_|\|_|_|_|_|_|_|_|
    // (the +1s are a "fencepost" phenomenon.) in the typical needleman-wunch            R |_|_|_|\|_|_|_|_|_|_|
    // algorithm, the alignments are scored by #matches - #mismatches.                   U |_|_|_|_|\|_|_|_|_|_|
    // (matching a real element with a filler is a mismatch.) in this case,              P |_|_|_|_|_|\|-|_|_|_|
    // an optimal prefix always leads to an optimal overall solution, and we             T |_|_|_|_|_|_|_|\|_|_|
    // can simply fill in the table with the optimal score up to that point              E |_|_|_|_|_|_|_|_|\|_|
    // and the "direction" of the last step taken to get there: vertical                 D |_|_|_|_|_|_|_|_|_|\|
    // if we matched an element of the second sequence with filler in the    an optimal path through this          PREDICTED
    // first sequence, horizontal if we matched an element of the first      matrix, describing the matching -->  CORRUP TED
    // sequence with a filler element of the second sequence, and diagonal   which has value 4-6 = -2, in contrast
    // if we matched an element of the first sequence with an element of     to the matching with no filler  -->   PREDICTED
    // the second sequence. (strictly speaking, if we are only interested    which has value 3-6 = -3.             CORRUPTED
    // in the quality of the alignment and not the optimal solution, we
    // don't need to store the directions; but they might end up being 
    // useful, so i do store them.)

    #[derive(Clone, Copy)]
    enum Direction {
        Vert,
        Diag,
        Horz,
    }
    
    // however, we don't want the standard needleman-wunch scoring, #matches - #mismatches;
    // instead we want the fraction of chords that are correct, #matches / (#matches + #mismatches).
    // unfortunately, this breaks the property that the best solution for the
    // whole thing builds on the best solution for the first part:
    //                       ____________
    //                      |            |
    //   BBBBA              | BBBBA      |
    //   ACCCC              |     ACCCC  |
    //   score: 0           | score: 1/9 |
    //  _______________     |____________|
    // |               |
    // | BBBBADDDDDDDD |       BBBBA    DDDDDDDD
    // | ACCCCDDDDDDDD |           ACCCCDDDDDDDD  
    // | score: 8/13   |        score: 9/17  
    // |_______________|
    //
    // however, we can still use a similar dynamic programming algorithm:
    // even though knowing the number of correct and number of incorrect elements in the first part
    // is insufficient to determine which will be the best path, we can still rule out some paths.
    // if we have partial solutions (a, b) and (c, d), if a <= c and b >= d, then (a, b) is definitely
    // no better than (c, d), since it has no more correct and no fewer incorrect.
    // so, we can just store a list of all the solutions that are on the convex hull of large a and small b.
    // for each value of a and each value of b there can be at most one pareto-optimal solution.
    // at index i,j, 0 <= a <= min(i,j) and max(i,j) <= b <= i+j-2a.
    // so, we need to store at most min(a,b) candidate solutions at i,j.
    // since a <= i <= n and a <= j <= m, the greatest number of candidate solutions we could need to store at
    // any index is min(n,m). so, the space (and time) complexity is O(n*m*min(n,m)).
    // this is no problem at all for any plausible values of n and m.

    let mut nw_matrix: Vec<Vec<Vec<(u8, u8, Direction)>>> = vec![vec![Vec::new(); seq_corrupted.len() + 1]; seq_predicted.len() + 1];
    for i in 0..seq_predicted.len() + 1 {
        for j in 0..seq_corrupted.len() + 1 {
            // the first row and column are initialized to describe the cost of inserting fillers at the start
            // of each sequence. the number of matches here is always zero, since it describes matching a
            // real element with a filler. the number of mismatches is just the number of fillers inserted.
            if i == 0 {
                nw_matrix[i][j].push((0, j as u8, Direction::Horz));
                // the direction at (0, 0) doesn't matter, so it's ok that we always set it to Horz
            } else if j == 0 {
                nw_matrix[i][j].push((0, i as u8, Direction::Vert));
            // the -1s are because the 0th element corresponds to the space before the sequence, not the first element of the sequence
            } else if seq_predicted[i - 1] == seq_corrupted[j - 1] {
                // in this case, the best thing to do is always to align these two elements, i.e. moving one
                // step forward in both the row and the column. (not aligning the elements may be equally good, but no better.)
                let (nw_toi, nw_pasti) = nw_matrix.split_at_mut(i);
                // these unwraps are safe because we initialized nw_matrix with its final dimensions (and an empty vector in each cell).
                // since j <= seq_corrupted.len(), the length of all the rows, the get_mut calls will never return None.
                let (nw_im1jm1, nw_ij) = (nw_toi[i-1].get_mut(j-1).unwrap(), nw_pasti[0].get_mut(j).unwrap());
                for (nc, ni, _) in nw_im1jm1 {
                    // since the solutions at (i-1, j-1) are all pareto-optimal, these are all pareto-optimal too
                    nw_ij.push((*nc + 1, *ni, Direction::Diag));
                }
            } else {
                // in this case, we need to consider the three options we have
                // (inserting a filler in either sequence or neither; equivalently, moving down, diagonal, or right to get here)
                // we will store all our candidate solutions indexed by the number of correct elements,
                // since we know this cannot exceed 2 * N_REPETITIONS_PER_TRIAL = 10.
                let mut candidates: HashMap<u8, (u8, Direction)> = HashMap::new();
                fn update_if_better(cd: &mut HashMap<u8, (u8, Direction)>, (nc, ni_new, dirn_new): (&u8, &u8, &Direction)) {
                    let _ = cd.insert(*nc, match cd.get(nc) {
                        Some((ni_old, dirn_old)) => if ni_new < ni_old { (*ni_new, *dirn_new) } else { (*ni_old, *dirn_old) },
                        None => (*ni_new, *dirn_new)
                    });
                }
                for (nc, ni, _) in &nw_matrix[i - 1][j] {
                    update_if_better(&mut candidates, (&nc, &(ni + 1), &Direction::Vert));
                }
                for (nc, ni, _) in &nw_matrix[i][j - 1] {
                    update_if_better(&mut candidates, (nc, &(ni + 1), &Direction::Horz));
                }
                for (nc, ni, _) in &nw_matrix[i - 1][j - 1] {
                    update_if_better(&mut candidates, (nc, &(ni + 1), &Direction::Diag));
                }
                // we've now found all the pareto-optimal solutions
                for (nc, (ni, dirn)) in candidates {
                    nw_matrix[i][j].push((nc, ni, dirn));
                }
            }
        }
    }
    // rank the elements of the last row by our desired metric, #matches / (#matches + #mismatches)
    let final_candidates = &nw_matrix[seq_predicted.len()][seq_corrupted.len()];
    // these two unwraps are safe: the first because the total number of elements is nonzero (it must be at least 2*N_REPETITIONS_PER_TRIAL),
    // the second because there is guaranteed to be at least one candidate solution.
    final_candidates.iter()
                    .map(|(nc, ni, _)| (*nc, *ni))
                    .max_by(|(nc1, ni1), (nc2, ni2)|
                                      ((*nc1 as f64) / ((*nc1 + *ni1) as f64))
                                      .partial_cmp(&((*nc2 as f64) / ((*nc2 + *ni2) as f64)))
                                      .unwrap())
                    .unwrap()                           
}

fn compute_accuracy<K: Key, const N: usize, L: Layout<K, N>>(actual_input: &str, chords: &[Chord<K, N, L>; 2]) -> f64 where Standard: Distribution<K> {
    let expected = get_expected_input(chords);
    // we define the keymap used when doing this exercise so that all combos end with ' '.
    // therefore, we can just split on spaces
    let actual = actual_input.split_whitespace().map(|s| s.to_string()).collect::<Vec<String>>();
    // now, we find the optimal "alignment" between the two sequences: the way to insert "filler" chords
    // in both of them so that the greatest number of chords match each other. 
    // i.e., for sequence ABABAB and BABABA, a direct comparison would give an accuracy of 0 but the optimal alignment     ABABAB
    // gives an accuracy of 5/7--after fillers are inserted, the sequence has length 7, and 5 of the chords match.         BABABA
    // (in other words, we assume that the user accidentally typed B before they attempted the sequence, and then missed the final element)
    // we don't give an ''partial credit'' if the user gets most of the keys in a chord right but messes up one or two; the result of this
    // will generally be illegible, so we want the reward model to learn to avoid chords which are difficult to type accurately.
    let expected_vec = expected.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    let (correct, incorrect) = alignment_quality(expected_vec, actual);
    (correct as f64) / ((correct + incorrect) as f64)
}


fn gather_data<K: Key, const N: usize, L: Layout<K, N>>() -> Result<TrialResults<K, N, L>, std::io::Error> where Standard: Distribution<K> {
    let mut rng = rand::thread_rng();
    println!("you will be shown two chords. after some time to practice, you will need to type this pair of chords {} times, as quickly as possible.", N_REPETITIONS_PER_TRIAL);
    
    let mut results: TrialResults<K, N, L> = TrialResults::new();

    // run trials until the user quits
    loop {
        let chords = [random_chord(&mut rng, CHORD_KEY_SAMPLE_THRESHOLD), random_chord(&mut rng, CHORD_KEY_SAMPLE_THRESHOLD)];
        for chord in &chords {
            println!("{}", chord);
        }

        'trial: loop {
            let mut practice_input = String::new();
            println!("type GO when you're ready to continue, IMP if this contains an impossible combination, SKIP to skip this pair without recording any data, or QUIT to quit. hit Enter after you're done typing the chords.");
            std::io::stdin().read_line(&mut practice_input)?;
            if practice_input == "GO\n" {
                let mut trial_input = String::new();
                let start_time = std::time::Instant::now();
                std::io::stdin().read_line(&mut trial_input)?;
                let trial_time = start_time.elapsed().as_secs_f64();
                let trial_accuracy = compute_accuracy(&trial_input, &chords);
                println!("expected input: {}; accuracy: {}; average switching time: {}", get_expected_input(&chords).join(" "), trial_accuracy, trial_time / ((2 * N_REPETITIONS_PER_TRIAL - 1) as f64));
                println!("accept this trial (Y), or try again (N)?");
                'accept: loop {
                    let mut accept_input = String::new();
                    std::io::stdin().read_line(&mut accept_input)?;
                    println!("");
                    if accept_input == "Y\n" {
                        let trial_data = TrialData {
                            chord_pair: chords,
                            n_repetitions: N_REPETITIONS_PER_TRIAL,
                            performance: Ok((trial_time, trial_accuracy)),
                        };
                        results.push(trial_data);
                        break 'trial;
                    } else if accept_input == "N\n" {
                        break 'accept;
                    } else {
                        println!("please type Y or N.");
                    }
                }
            } else if practice_input == "SKIP\n" {
                break 'trial;
            } else if practice_input == "IMP\n" {
                let trial_data = TrialData {
                    chord_pair: chords,
                    n_repetitions: N_REPETITIONS_PER_TRIAL,
                    performance: Err(ErrCode::Impossible),
                };
                results.push(trial_data);
                break 'trial;
            } else if practice_input == "QUIT\n" {
                println!("quitting...");
                return Ok(results);
            }
        }


    }
}

pub fn gather_and_save_data<K: Key, const N: usize, L: Layout<K, N>>(filename: &str) -> Result<TrialResults<K, N, L>, std::io::Error> where Standard: Distribution<K> {
    let results = gather_data::<K, N, L>()?;
    results.save(filename)?;
    Ok(results)
}

pub fn run<K: Key, const N: usize, L: Layout<K, N>>() where Standard: Distribution<K> {
    let results_path = format!("{}/chord_preferences_results_{}.json",
                                       RESULTS_PATH,
                                       std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());

    match gather_and_save_data::<K, N, L>(&results_path) {
        Ok(gather_results) => gather_results,
        Err(e) => {
            eprintln!("Error gathering or saving data: {}", e);
            return;
        }
    };
}
