use rand::distributions::{Distribution, Standard};
use rand::prelude::SliceRandom;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use std::{array, vec};
use std::collections::HashMap;

use crate::keyboard_config::{Key, Chord, Layout, ChordTrialUtils, GraphicalChord};
use crate::local_env::RESULTS_PATH;

const N_REPETITIONS_PER_TRIAL: usize = 5;

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
    pub input: Result<Vec<Chord<K, N, L>>, ErrCode>,  // the first element is the total time, the second is the accuracy
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

pub fn alignment_quality<T: PartialEq>(seq_predicted: &Vec<T>, seq_corrupted: &Vec<T>) -> (u8, u8) {
    // returns the number of correct chords and the number of incorrect chords after alignment.
    let (correct, incorrect, _) = align(seq_predicted, seq_corrupted);
    (correct, incorrect)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Direction {
    Vert,
    Diag,
    Horz,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Direction::Vert => write!(f, "|"),
            Direction::Diag => write!(f, "\\"),
            Direction::Horz => write!(f, "-"),
        }
    }
}

pub fn best_candidate(candidates: &Vec<(u8, u8, Direction)>) -> &(u8, u8, Direction) {
    // these two unwraps are safe: the first because the total number of elements is nonzero (it must be at least 2*N_REPETITIONS_PER_TRIAL),
    // so the partial_cmp will never fail due to zero division;
    // the second because there is guaranteed to be at least one candidate solution.
    candidates.iter()
              .max_by(|(nc1, ni1, _), (nc2, ni2, _)| ((*nc1 as f64) / ((*nc1 + *ni1) as f64))
                                                     .partial_cmp(&((*nc2 as f64) / ((*nc2 + *ni2) as f64)))
                                                     .unwrap())
              .unwrap()
}

pub fn align<T: PartialEq>(seq_predicted: &Vec<T>, seq_corrupted: &Vec<T>) -> (u8, u8, Vec<Vec<Vec<(u8, u8, Direction)>>>) {
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

    
    // however, we don't want the standard needleman-wunch scoring, #matches - #mismatches;
    // instead we want the fraction of chords that are correct, #matches / (#matches + #mismatches).
    // (we also will count multiple insertions of the same chord as a single error, since this is
    // usually caused by holding a key down incorrectly.)

    const COUNT_MULTIPLE_INSERTIONS_ONCE: bool = true;

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
                if j > 1 && j < seq_corrupted.len() && COUNT_MULTIPLE_INSERTIONS_ONCE && seq_corrupted[j - 2] == seq_corrupted[j - 1] {
                    // if the user types the same chord twice in a row, we only count this as one error

                    // split off nw_matrix[0][j-1] from [0][j] so we can borrow the former immutably and the latter mutably
                    let (nw_pre_j, nw_post_j) = nw_matrix[0].split_at_mut(j);

                    // for all the options (in fact there is only one) in the previous column, we add an option to this column
                    // with the same number of incorrect elements so the insertion is only counted once
                    for (_, ni, _) in &nw_pre_j[j-1] {
                        nw_post_j[0].push((0, *ni, Direction::Horz));
                    }
                } else {
                    nw_matrix[i][j].push((0, j as u8, Direction::Horz));
                }
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
                    if 1 < j && j < seq_corrupted.len() && COUNT_MULTIPLE_INSERTIONS_ONCE && seq_corrupted[j - 1] == seq_corrupted[j - 2] {
                        update_if_better(&mut candidates, (nc, ni, &Direction::Horz));
                    } else {
                        update_if_better(&mut candidates, (nc, &(ni + 1), &Direction::Horz));
                    }
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
    let (correct, incorrect, _) = best_candidate(final_candidates);
    (*correct, *incorrect, nw_matrix)
}

fn compute_accuracy<K: Key, const N: usize, L: Layout<K, N>>(actual_input: &Vec<Chord<K, N, L>>, expected_input: &Vec<Chord<K, N, L>>) -> f64 where Standard: Distribution<K> {
    // we find the optimal "alignment" between the two sequences: the way to insert "filler" chords
    // in both of them so that the greatest number of chords match each other. 
    // i.e., for sequence ABABAB and BABABA, a direct comparison would give an accuracy of 0 but the optimal alignment     ABABAB
    // gives an accuracy of 5/7--after fillers are inserted, the sequence has length 7, and 5 of the chords match.         BABABA
    // (in other words, we assume that the user accidentally typed B before they attempted the sequence, and then missed the final element)
    // we don't give an ''partial credit'' if the user gets most of the keys in a chord right but messes up one or two; the result of this
    // will generally be illegible, so we want the reward model to learn to avoid chords which are difficult to type accurately.
    let (correct, incorrect) = alignment_quality(expected_input, actual_input);
    (correct as f64) / ((correct + incorrect) as f64)
}

fn gather_data<K: Key, const N: usize, L: Layout<K, N>, C: ChordTrialUtils<K, N, L>>(chord_trial_utils: C) -> Result<TrialResults<K, N, L>, std::io::Error> where Standard: Distribution<K> {
    let rng = &mut rand::thread_rng();
    println!("you will be shown two chords. after some time to practice, you will need to type this pair of chords {} times, as quickly as possible.", N_REPETITIONS_PER_TRIAL);
    
    let mut results: TrialResults<K, N, L> = TrialResults::new();

    let chord_list: Vec<&Chord<K, N, L>> = chord_trial_utils.get_vocab()
                                                           .into_iter()
                                                           .map(|(chord, _)| chord)
                                                           .collect();

    // run trials until the user quits
    loop {
        // the unwraps are safe because chord_list is nonempty
        let chords: [Chord<K, N, L>; 2] = [(**chord_list.choose(rng).unwrap()).clone(),
                                           (**chord_list.choose(rng).unwrap()).clone()];
        for chord in &chords {
            println!("{}", GraphicalChord { chord });
        }

        'trial: loop {
            let mut practice_input = String::new();
            println!("type GO when you're ready to continue, IMP if this contains an impossible combination, SKIP to skip this pair without recording any data, or QUIT to quit. hit Enter after you're done typing the chords.");
            std::io::stdin().read_line(&mut practice_input)?;
            if practice_input == "GO\n" {
                let mut trial_input = String::new();
                let start_time = std::time::Instant::now();
                std::io::stdin().read_line(&mut trial_input)?;
                let parsed_chords = match chord_trial_utils.parse_trial_string(&trial_input[0..trial_input.len() - 1]) {  // remove the final newline
                    Ok(parsed) => parsed,
                    Err(e) => {
                        println!("error parsing input: {}. perhaps you entered text from the wrong device?", e);
                        continue 'trial;
                    }
                };
                let trial_time = start_time.elapsed().as_secs_f64();

                // print accuracy and speed to the user
                let expected_chords: [Chord<K, N, L>; 2 * N_REPETITIONS_PER_TRIAL] = array::from_fn(|i| chords[i % 2].clone());
                let trial_accuracy = compute_accuracy::<K, N, L>(&parsed_chords, &expected_chords.to_vec());
                let expected_input: Vec<String> = expected_chords.into_iter().map(|c| chord_trial_utils.lookup_chord(&c).unwrap()).collect();  // this unwrap is safe if the code is correct, because this chord belongs to the vocab
                println!("expected input: {}; accuracy: {}; average switching time: {}", expected_input.join(" "), trial_accuracy, trial_time / ((2 * N_REPETITIONS_PER_TRIAL - 1) as f64));
                println!("accept this trial (Y), or try again (N)?");
                'accept: loop {
                    let mut accept_input = String::new();
                    std::io::stdin().read_line(&mut accept_input)?;
                    println!("");
                    if accept_input == "Y\n" {
                        let trial_data = TrialData {
                            chord_pair: chords,
                            n_repetitions: N_REPETITIONS_PER_TRIAL,
                            input: Ok(parsed_chords),
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
                    input: Err(ErrCode::Impossible),
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

pub fn gather_and_save_data<K: Key, const N: usize, L: Layout<K, N>, C: ChordTrialUtils<K, N, L>>(chord_trial_utils_file: &str) -> Result<TrialResults<K, N, L>, std::io::Error> where Standard: Distribution<K> {
    let results_path = format!("{}/chord_preferences_results_{}.json",
                                       RESULTS_PATH,
                                       std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
    let chord_trial_utils: C = serde_json::from_reader(std::fs::File::open(std::path::Path::new(chord_trial_utils_file))?)?;
    let results = gather_data::<K, N, L, C>(chord_trial_utils)?;
    results.save(&results_path)?;
    Ok(results)
}

pub fn run<K: Key, const N: usize, L: Layout<K, N>, C: ChordTrialUtils<K, N, L>>(chord_trial_utils_file: &str) where Standard: Distribution<K> {
    match gather_and_save_data::<K, N, L, C>(chord_trial_utils_file) {
        Ok(gather_results) => gather_results,
        Err(e) => {
            eprintln!("Error gathering or saving data: {}", e);
            return;
        }
    };
}
