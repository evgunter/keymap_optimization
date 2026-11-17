//! Timing trials: alternating between two chords and recording when each chord arrives.

use std::time::{Duration, SystemTime};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::chord::Chord;
use crate::deck::Deck;
use crate::reader::TimedKeystroke;

/// How many times the pair is pressed correctly in one timed run.
pub const CYCLES: usize = 5;
pub const PRESSES_PER_RUN: usize = 2 * CYCLES;

/// How a timed run decides which chord it is waiting for and when it is over. It is recorded with every attempt,
/// so that attempts made under different rules cannot be mistaken for one another.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Rule {
    /// The run waits for the chords of the pair in turn. A wrong chord does not move it on: the chord it is
    /// waiting for has to be pressed until it arrives. The run ends after `PRESSES_PER_RUN` correct chords.
    RepeatUntilCorrect,
}

/// For each chord that arrived in a run, whether it was the one the run was waiting for.
pub fn correct_presses(pair: &[Chord; 2], chords: impl IntoIterator<Item = Option<Chord>>) -> Vec<bool> {
    let mut n_correct = 0;
    chords.into_iter().map(|chord| {
        let correct = chord == Some(pair[n_correct % 2]);
        if correct {
            n_correct += 1;
        }
        correct
    }).collect()
}

/// How far the system clock may drift from a monotonic clock over a run before the run is thrown away.
/// It has to allow for the varying delay between the kernel and this program receiving a keystroke.
const CLOCK_TOLERANCE: Duration = Duration::from_millis(20);

/// One chord arriving during a timed run.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Press {
    /// The chord that arrived; `None` for a keystroke that no chord in the deck sends.
    pub chord: Option<Chord>,
    /// Seconds since the first press of the run.
    pub seconds: f64,
}

/// One timed run of a trial.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Attempt {
    /// The position of the trial in the plan.
    pub trial: usize,
    /// The chords to alternate between, starting with the first.
    pub pair: [Chord; 2],
    /// Every chord that arrived, whether or not it was the right one.
    pub presses: Vec<Press>,
    /// When the first press arrived.
    pub start: SystemTime,
    /// Whether the run was given up in favour of another attempt at the same trial.
    pub redone: bool,
    pub rule: Rule,
}

#[derive(Debug, PartialEq, thiserror::Error)]
#[error("the system clock was adjusted during the run")]
pub struct ClockUnsteady;

/// The seconds from the first keystroke to each keystroke, by the kernel's timestamps.
fn seconds_since_first(keystrokes: &[TimedKeystroke]) -> Result<Vec<f64>, ClockUnsteady> {
    let Some(first) = keystrokes.first() else { return Ok(Vec::new()) };
    keystrokes.iter().map(|keystroke| {
        let by_system_clock = keystroke.time.duration_since(first.time).map_err(|_| ClockUnsteady)?;
        let by_monotonic_clock = keystroke.received.duration_since(first.received);
        if by_system_clock.abs_diff(by_monotonic_clock) > CLOCK_TOLERANCE {
            Err(ClockUnsteady)
        } else {
            Ok(by_system_clock.as_secs_f64())
        }
    }).collect()
}

/// What a run shows about the two directions of switching.
#[derive(Clone, PartialEq, Debug)]
pub struct Summary {
    /// How many chords arrived that were not the one the run was waiting for.
    pub n_wrong: usize,
    /// The seconds from each correct chord to the next correct chord, including any wrong chords in between:
    /// first those from the first chord of the pair to the second, then those from the second to the first.
    pub switches: [Vec<f64>; 2],
}

pub fn summarize(pair: &[Chord; 2], presses: &[Press]) -> Summary {
    let correct = correct_presses(pair, presses.iter().map(|press| press.chord));
    let correct_seconds = presses.iter().zip(&correct).filter(|(_, correct)| **correct).map(|(press, _)| press.seconds);
    let mut switches = [Vec::new(), Vec::new()];
    for (index, (from, to)) in correct_seconds.tuple_windows().enumerate() {
        switches[index % 2].push(to - from);
    }
    Summary { n_wrong: correct.iter().filter(|correct| !**correct).count(), switches }
}

pub fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    match sorted.len() {
        0 => None,
        n if n % 2 == 1 => Some(sorted[middle]),
        _ => Some((sorted[middle - 1] + sorted[middle]) / 2.0),
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum SessionError {
    #[error("trial {0} of the plan alternates a chord with itself")]
    SameChordTwice(usize),
    #[error("the plan uses {0:?}, which is not in the deck")]
    ChordNotInDeck(Chord),
    #[error("record {0} of the results does not continue the plan; are these the results of a different plan?")]
    ResultsDoNotMatchPlan(usize),
}

/// What the session is waiting for.
#[derive(Clone, PartialEq, Debug)]
pub enum Step {
    /// The pair can be practised until the timed run is started.
    Practice { trial: usize, pair: [Chord; 2] },
    /// A timed run is under way and has received this many correct chords. `mistake` is set if the chord that
    /// arrived last was not the one the run was waiting for.
    Run { trial: usize, pair: [Chord; 2], n_correct: usize, mistake: Option<Mistake> },
    /// A timed run is complete and should be accepted or redone.
    Review { trial: usize, pair: [Chord; 2], summary: Summary },
    Finished,
}

/// A chord that arrived in a run when a different one was awaited.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mistake {
    /// The chord that arrived; `None` for a keystroke that no chord in the deck sends.
    pub registered: Option<Chord>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    /// Start the timed run, or accept the completed one.
    Proceed,
    /// Give up the timed run, under way or completed, and go back to practising the same pair.
    Redo,
}

/// How the session reacted to an input.
#[derive(Clone, PartialEq, Debug)]
pub enum Response {
    /// The step changed without anything to save.
    Moved,
    /// This chord arrived during practice.
    Practised(Option<Chord>),
    /// A timed run ended and should be saved.
    Ended(Attempt),
    /// A timed run was thrown away because its timestamps cannot be trusted.
    ClockUnsteady,
    /// The input has no meaning at the current step.
    Ignored,
}

enum Phase {
    Practice,
    Run(Vec<(Option<Chord>, TimedKeystroke)>),
    Review { presses: Vec<Press>, start: SystemTime },
}

/// One sitting of timing trials, following a plan.
pub struct Session {
    deck: Deck,
    plan: Vec<[Chord; 2]>,
    /// The position in the plan of the trial being worked on.
    trial: usize,
    phase: Phase,
}

impl Session {
    /// `attempts` are the attempts already made at this plan, oldest first.
    pub fn new(deck: Deck, plan: Vec<[Chord; 2]>, attempts: &[Attempt]) -> Result<Self, SessionError> {
        for (index, pair) in plan.iter().enumerate() {
            if pair[0] == pair[1] {
                return Err(SessionError::SameChordTwice(index));
            }
            if let Some(chord) = pair.iter().find(|chord| !deck.chords().contains(chord)) {
                return Err(SessionError::ChordNotInDeck(*chord));
            }
        }
        // every attempt is at the first trial that has no accepted attempt yet
        let mut trial = 0;
        for (index, attempt) in attempts.iter().enumerate() {
            if attempt.trial != trial || plan.get(trial) != Some(&attempt.pair) {
                return Err(SessionError::ResultsDoNotMatchPlan(index));
            }
            if !attempt.redone {
                trial += 1;
            }
        }
        Ok(Session { deck, plan, trial, phase: Phase::Practice })
    }

    pub fn n_trials(&self) -> usize {
        self.plan.len()
    }

    pub fn step(&self) -> Step {
        let Some(&pair) = self.plan.get(self.trial) else { return Step::Finished };
        let trial = self.trial;
        match &self.phase {
            Phase::Practice => Step::Practice { trial, pair },
            Phase::Run(presses) => {
                let correct = correct_presses(&pair, presses.iter().map(|(chord, _)| *chord));
                let mistake = match (presses.last(), correct.last()) {
                    (Some((registered, _)), Some(false)) => Some(Mistake { registered: *registered }),
                    _ => None,
                };
                Step::Run { trial, pair, n_correct: correct.iter().filter(|correct| **correct).count(), mistake }
            }
            Phase::Review { presses, .. } => Step::Review { trial, pair, summary: summarize(&pair, presses) },
        }
    }

    fn attempt(&self, presses: Vec<Press>, start: SystemTime, redone: bool) -> Attempt {
        Attempt { trial: self.trial, pair: self.plan[self.trial], presses, start, redone, rule: Rule::RepeatUntilCorrect }
    }

    pub fn handle_keystroke(&mut self, timed: TimedKeystroke) -> Response {
        if self.trial >= self.plan.len() {
            return Response::Ignored;
        }
        let chord = self.deck.chord_for(timed.keystroke);
        let pair = self.plan[self.trial];
        match &mut self.phase {
            Phase::Practice => Response::Practised(chord),
            Phase::Run(presses) => {
                presses.push((chord, timed));
                let n_correct = correct_presses(&pair, presses.iter().map(|(chord, _)| *chord)).into_iter().filter(|correct| *correct).count();
                if n_correct < PRESSES_PER_RUN {
                    return Response::Moved;
                }
                let (chords, keystrokes): (Vec<Option<Chord>>, Vec<TimedKeystroke>) = presses.drain(..).unzip();
                match seconds_since_first(&keystrokes) {
                    Ok(seconds) => {
                        let presses = chords.into_iter().zip(seconds).map(|(chord, seconds)| Press { chord, seconds }).collect();
                        self.phase = Phase::Review { presses, start: keystrokes[0].time };
                        Response::Moved
                    }
                    Err(ClockUnsteady) => {
                        self.phase = Phase::Practice;
                        Response::ClockUnsteady
                    }
                }
            }
            Phase::Review { .. } => Response::Ignored,
        }
    }

    pub fn handle_command(&mut self, command: Command) -> Response {
        if self.trial >= self.plan.len() {
            return Response::Ignored;
        }
        match (std::mem::replace(&mut self.phase, Phase::Practice), command) {
            (Phase::Practice, Command::Proceed) => {
                self.phase = Phase::Run(Vec::new());
                Response::Moved
            }
            (Phase::Practice, Command::Redo) => Response::Ignored,
            (Phase::Run(presses), Command::Proceed) => {
                self.phase = Phase::Run(presses);
                Response::Ignored
            }
            (Phase::Run(presses), Command::Redo) => {
                let (chords, keystrokes): (Vec<Option<Chord>>, Vec<TimedKeystroke>) = presses.into_iter().unzip();
                match (keystrokes.first(), seconds_since_first(&keystrokes)) {
                    // nothing was pressed, so there is nothing to save
                    (None, _) => Response::Moved,
                    (Some(_), Err(ClockUnsteady)) => Response::ClockUnsteady,
                    (Some(first), Ok(seconds)) => {
                        let presses = chords.into_iter().zip(seconds).map(|(chord, seconds)| Press { chord, seconds }).collect();
                        Response::Ended(self.attempt(presses, first.time, true))
                    }
                }
            }
            (Phase::Review { presses, start }, Command::Proceed) => {
                let attempt = self.attempt(presses, start, false);
                self.trial += 1;
                Response::Ended(attempt)
            }
            (Phase::Review { presses, start }, Command::Redo) => Response::Ended(self.attempt(presses, start, true)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use crate::chord::{FingerState::{self, Lifted, L, M, R}, ThumbState};
    use crate::keystroke::Keystroke;

    fn chord(fingers: [FingerState; 4]) -> Chord {
        Chord::new(ThumbState::Lifted, fingers).unwrap()
    }

    fn chords() -> [Chord; 3] {
        [chord([L, Lifted, Lifted, Lifted]), chord([M, Lifted, Lifted, Lifted]), chord([R, Lifted, Lifted, Lifted])]
    }

    fn deck() -> Deck {
        Deck::new(chords().to_vec()).unwrap()
    }

    /// Makes keystrokes for the deck's chords by their position, at times given in milliseconds on both clocks.
    struct Clock {
        system: SystemTime,
        monotonic: Instant,
    }

    impl Clock {
        fn new() -> Self {
            Clock { system: SystemTime::now(), monotonic: Instant::now() }
        }

        fn keystroke_at(&self, chord_index: usize, system_ms: u64, monotonic_ms: u64) -> TimedKeystroke {
            TimedKeystroke {
                time: self.system + Duration::from_millis(system_ms),
                received: self.monotonic + Duration::from_millis(monotonic_ms),
                keystroke: Keystroke::from_index(chord_index).unwrap(),
            }
        }

        fn keystroke(&self, chord_index: usize, ms: u64) -> TimedKeystroke {
            self.keystroke_at(chord_index, ms, ms)
        }
    }

    /// Presses the first two chords of the deck alternately, 100 ms apart, for a whole run.
    fn complete_run(session: &mut Session, clock: &Clock) {
        assert_eq!(session.handle_command(Command::Proceed), Response::Moved);
        for index in 0..PRESSES_PER_RUN {
            assert_eq!(session.handle_keystroke(clock.keystroke(index % 2, 100 * index as u64)), Response::Moved);
        }
    }

    fn plan() -> Vec<[Chord; 2]> {
        let [a, b, c] = chords();
        vec![[a, b], [a, c]]
    }

    #[test]
    fn a_clean_run_is_reviewed_then_saved_and_the_next_trial_starts() {
        let [a, b, c] = chords();
        let clock = Clock::new();
        let mut session = Session::new(deck(), plan(), &[]).unwrap();
        assert_eq!(session.step(), Step::Practice { trial: 0, pair: [a, b] });
        complete_run(&mut session, &clock);

        let Step::Review { trial: 0, summary, .. } = session.step() else { panic!("expected a review, got {:?}", session.step()) };
        assert_eq!(summary.n_wrong, 0);
        assert_eq!(summary.switches.clone().map(|switches| switches.len()), [CYCLES, CYCLES - 1]);
        assert!(summary.switches.iter().flatten().all(|seconds| (seconds - 0.1).abs() < 1e-9));

        let Response::Ended(attempt) = session.handle_command(Command::Proceed) else { panic!("expected the attempt") };
        assert_eq!((attempt.trial, attempt.pair, attempt.redone, attempt.start), (0, [a, b], false, clock.system));
        assert_eq!(attempt.rule, Rule::RepeatUntilCorrect);
        assert_eq!(attempt.presses.len(), PRESSES_PER_RUN);
        assert_eq!(attempt.presses[3], Press { chord: Some(b), seconds: 0.3 });
        assert_eq!(session.step(), Step::Practice { trial: 1, pair: [a, c] });
    }

    #[test]
    fn a_redone_run_is_saved_as_redone_and_the_trial_stays() {
        let [a, b, _] = chords();
        let clock = Clock::new();
        let mut session = Session::new(deck(), plan(), &[]).unwrap();
        complete_run(&mut session, &clock);
        let Response::Ended(attempt) = session.handle_command(Command::Redo) else { panic!("expected the attempt") };
        assert!(attempt.redone);
        assert_eq!(attempt.presses.len(), PRESSES_PER_RUN);
        assert_eq!(session.step(), Step::Practice { trial: 0, pair: [a, b] });
    }

    #[test]
    fn giving_up_a_run_under_way_saves_what_was_pressed() {
        let [a, b, _] = chords();
        let clock = Clock::new();
        let mut session = Session::new(deck(), plan(), &[]).unwrap();
        session.handle_command(Command::Proceed);
        assert_eq!(session.handle_command(Command::Redo), Response::Moved);

        session.handle_command(Command::Proceed);
        session.handle_keystroke(clock.keystroke(0, 0));
        session.handle_keystroke(clock.keystroke(2, 150));
        assert_eq!(session.step(), Step::Run { trial: 0, pair: [a, b], n_correct: 1, mistake: Some(Mistake { registered: Some(chords()[2]) }) });
        let Response::Ended(attempt) = session.handle_command(Command::Redo) else { panic!("expected the attempt") };
        assert!(attempt.redone);
        assert_eq!(attempt.presses, [Press { chord: Some(a), seconds: 0.0 }, Press { chord: Some(chords()[2]), seconds: 0.15 }]);
        assert_eq!(session.step(), Step::Practice { trial: 0, pair: [a, b] });
    }

    #[test]
    fn practice_presses_are_reported_and_not_recorded() {
        let clock = Clock::new();
        let mut session = Session::new(deck(), plan(), &[]).unwrap();
        assert_eq!(session.handle_keystroke(clock.keystroke(1, 0)), Response::Practised(Some(chords()[1])));
        assert_eq!(session.handle_keystroke(clock.keystroke(7, 0)), Response::Practised(None));
        assert!(matches!(session.step(), Step::Practice { trial: 0, .. }));
    }

    #[test]
    fn a_run_during_which_the_clock_jumped_is_thrown_away() {
        let clock = Clock::new();
        let mut session = Session::new(deck(), plan(), &[]).unwrap();
        session.handle_command(Command::Proceed);
        for index in 0..PRESSES_PER_RUN - 1 {
            session.handle_keystroke(clock.keystroke(index % 2, 100 * index as u64));
        }
        // the system clock says 5 s have passed, the monotonic clock 0.9 s
        assert_eq!(session.handle_keystroke(clock.keystroke_at(1, 5000, 900)), Response::ClockUnsteady);
        assert!(matches!(session.step(), Step::Practice { trial: 0, .. }));
    }

    #[test]
    fn clocks_may_disagree_slightly_but_not_much_or_backwards() {
        let clock = Clock::new();
        let seconds = seconds_since_first(&[clock.keystroke(0, 0), clock.keystroke_at(1, 300, 310)]).unwrap();
        assert_eq!(seconds, [0.0, 0.3]);
        assert_eq!(seconds_since_first(&[clock.keystroke(0, 0), clock.keystroke_at(1, 300, 330)]), Err(ClockUnsteady));
        assert_eq!(seconds_since_first(&[clock.keystroke(0, 100), clock.keystroke_at(1, 0, 400)]), Err(ClockUnsteady));
        assert_eq!(seconds_since_first(&[]), Ok(Vec::new()));
    }

    #[test]
    fn a_wrong_chord_does_not_move_the_run_on() {
        let [a, b, c] = chords();
        let clock = Clock::new();
        let mut session = Session::new(deck(), plan(), &[]).unwrap();
        session.handle_command(Command::Proceed);
        // the wrong chord, an unknown keystroke, and the awaited chord's partner each leave the run waiting for b
        let wrong_presses = [(2, Some(c)), (7, None), (0, Some(a))];
        session.handle_keystroke(clock.keystroke(0, 0));
        for (index, (chord_index, registered)) in wrong_presses.into_iter().enumerate() {
            assert_eq!(session.handle_keystroke(clock.keystroke(chord_index, 100 * (index as u64 + 1))), Response::Moved);
            assert_eq!(session.step(), Step::Run { trial: 0, pair: [a, b], n_correct: 1, mistake: Some(Mistake { registered }) });
        }
        session.handle_keystroke(clock.keystroke(1, 400));
        assert_eq!(session.step(), Step::Run { trial: 0, pair: [a, b], n_correct: 2, mistake: None });

        // the run ends only once every chord of the alternation has arrived
        for index in 2..PRESSES_PER_RUN {
            assert!(matches!(session.step(), Step::Run { .. }));
            session.handle_keystroke(clock.keystroke(index % 2, 500 + 100 * index as u64));
        }
        let Step::Review { summary, .. } = session.step() else { panic!("expected a review, got {:?}", session.step()) };
        assert_eq!(summary.n_wrong, 3);
        let Response::Ended(attempt) = session.handle_command(Command::Proceed) else { panic!("expected the attempt") };
        assert_eq!(attempt.presses.len(), PRESSES_PER_RUN + 3);
        assert_eq!(attempt.presses[2], Press { chord: None, seconds: 0.2 });
    }

    #[test]
    fn correct_presses_follow_the_alternation() {
        let [a, b, c] = chords();
        let arrived = [Some(a), Some(a), Some(b), Some(c), None, Some(b), Some(a), Some(b)];
        assert_eq!(correct_presses(&[a, b], arrived), [true, false, true, false, false, false, true, true]);
        // a run has to start with the first chord of the pair
        assert_eq!(correct_presses(&[a, b], [Some(b), Some(a)]), [false, true]);
    }

    #[test]
    fn summary_times_each_switch_up_to_the_correct_chord() {
        let [a, b, c] = chords();
        let press = |chord, seconds| Press { chord, seconds };
        // a wrong chord before the second b, and an unknown keystroke and a repeated a before the last b
        let presses = [press(Some(a), 0.0), press(Some(b), 0.2), press(Some(a), 0.5), press(Some(c), 0.6), press(Some(b), 0.9),
                       press(Some(a), 1.0), press(None, 1.1), press(Some(a), 1.3), press(Some(b), 1.5)];
        let summary = summarize(&[a, b], &presses);
        assert_eq!(summary.n_wrong, 3);
        let close = |actual: &[f64], expected: &[f64]| actual.len() == expected.len() && actual.iter().zip(expected).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(close(&summary.switches[0], &[0.2, 0.4, 0.5]), "{:?}", summary.switches[0]);
        assert!(close(&summary.switches[1], &[0.3, 0.1]), "{:?}", summary.switches[1]);
    }

    #[test]
    fn median_handles_odd_even_and_empty() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn a_session_resumes_after_the_accepted_attempts() {
        let [a, b, c] = chords();
        let attempt = |trial, pair, redone| Attempt { trial, pair, presses: Vec::new(), start: SystemTime::now(), redone, rule: Rule::RepeatUntilCorrect };
        let session = Session::new(deck(), plan(), &[attempt(0, [a, b], true), attempt(0, [a, b], false), attempt(1, [a, c], true)]).unwrap();
        assert_eq!(session.step(), Step::Practice { trial: 1, pair: [a, c] });

        let mut finished = Session::new(deck(), plan(), &[attempt(0, [a, b], false), attempt(1, [a, c], false)]).unwrap();
        assert_eq!(finished.step(), Step::Finished);
        assert_eq!(finished.handle_command(Command::Proceed), Response::Ignored);
        assert_eq!(finished.handle_keystroke(Clock::new().keystroke(0, 0)), Response::Ignored);
    }

    #[test]
    fn results_of_another_plan_are_rejected() {
        let [a, b, c] = chords();
        let attempt = |trial, pair, redone| Attempt { trial, pair, presses: Vec::new(), start: SystemTime::now(), redone, rule: Rule::RepeatUntilCorrect };
        let rejected = |attempts: &[Attempt]| Session::new(deck(), plan(), attempts).err();
        // the wrong pair, a skipped trial, a repeated trial, and more trials than the plan has
        assert_eq!(rejected(&[attempt(0, [a, c], false)]), Some(SessionError::ResultsDoNotMatchPlan(0)));
        assert_eq!(rejected(&[attempt(1, [a, c], false)]), Some(SessionError::ResultsDoNotMatchPlan(0)));
        assert_eq!(rejected(&[attempt(0, [a, b], false), attempt(0, [a, b], false)]), Some(SessionError::ResultsDoNotMatchPlan(1)));
        assert_eq!(rejected(&[attempt(0, [a, b], false), attempt(1, [a, c], false), attempt(2, [a, b], false)]), Some(SessionError::ResultsDoNotMatchPlan(2)));
    }

    #[test]
    fn invalid_plans_are_rejected() {
        let [a, b, _] = chords();
        assert_eq!(Session::new(deck(), vec![[a, a]], &[]).err(), Some(SessionError::SameChordTwice(0)));
        let small_deck = Deck::new(vec![a]).unwrap();
        assert_eq!(Session::new(small_deck, vec![[a, b]], &[]).err(), Some(SessionError::ChordNotInDeck(b)));
    }

    #[test]
    fn inputs_that_do_not_apply_are_ignored() {
        let clock = Clock::new();
        let mut session = Session::new(deck(), plan(), &[]).unwrap();
        assert_eq!(session.handle_command(Command::Redo), Response::Ignored);
        session.handle_command(Command::Proceed);
        assert_eq!(session.handle_command(Command::Proceed), Response::Ignored);
        assert!(matches!(session.step(), Step::Run { n_correct: 0, mistake: None, .. }));
        session.handle_command(Command::Redo);
        complete_run(&mut session, &clock);
        assert_eq!(session.handle_keystroke(clock.keystroke(0, 5000)), Response::Ignored);
        assert!(matches!(session.step(), Step::Review { .. }));
    }

    #[test]
    fn attempts_recorded_without_a_rule_are_rejected() {
        let chord = r#"{"thumb":"Lifted","fingers":["L","Lifted","Lifted","Lifted"]}"#;
        let attempt = format!(r#"{{"trial":0,"pair":[{chord},{chord}],"presses":[],"start":{{"secs_since_epoch":0,"nanos_since_epoch":0}},"redone":false}}"#);
        assert!(serde_json::from_str::<Attempt>(&attempt).is_err());
        let with_rule = attempt.replace(r#""redone":false"#, r#""redone":false,"rule":"RepeatUntilCorrect""#);
        assert!(serde_json::from_str::<Attempt>(&with_rule).is_ok());
    }
}
