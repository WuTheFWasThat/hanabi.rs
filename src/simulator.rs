use rand::{self, Rng, SeedableRng};
use fnv::FnvHashMap;
use std::fmt;
use crossbeam;

use game::*;
use strategy::*;

fn new_deck(seed: u32) -> Cards {
    let mut deck: Cards = Cards::new();

    for &color in COLORS.iter() {
        for &value in VALUES.iter() {
            for _ in 0..get_count_for_value(value) {
                deck.push(Card::new(color, value));
            }
        }
    };

    rand::ChaChaRng::from_seed(&[seed]).shuffle(&mut deck[..]);
    debug!("Deck: {:?}", deck);
    deck
}

pub fn simulate_once(
        opts: &GameOptions,
        game_strategy: Box<GameStrategy>,
        seed: u32,
    ) -> GameState {
    let deck = new_deck(seed);

    let mut game = GameState::new(opts, deck);

    let mut strategies = game.get_players().map(|player| {
        (player, game_strategy.initialize(player, &game.get_view(player)))
    }).collect::<FnvHashMap<Player, Box<PlayerStrategy>>>();

    while !game.is_over() {
        let player = game.board.player;

        debug!("");
        debug!("=======================================================");
        debug!("Turn {}, Player {} to go", game.board.turn, player);
        debug!("=======================================================");
        debug!("{}", game);


        let choice = {
            let mut strategy = strategies.get_mut(&player).unwrap();
            strategy.decide(&game.get_view(player))
        };

        let turn = game.process_choice(choice);

        for player in game.get_players() {
            let mut strategy = strategies.get_mut(&player).unwrap();
            strategy.update(&turn, &game.get_view(player));
        }

    }
    debug!("");
    debug!("=======================================================");
    debug!("Final state:\n{}", game);
    debug!("SCORE: {:?}", game.score());
    game
}

#[derive(Debug)]
pub struct Histogram {
    pub hist: FnvHashMap<Score, u32>,
    pub sum: Score,
    pub total_count: u32,
}
impl Histogram {
    pub fn new() -> Histogram {
        Histogram {
            hist: FnvHashMap::default(),
            sum: 0,
            total_count: 0,
        }
    }
    fn insert_many(&mut self, val: Score, count: u32) {
        let new_count = self.get_count(&val) + count;
        self.hist.insert(val, new_count);
        self.sum += val * (count as u32);
        self.total_count += count;
    }
    pub fn insert(&mut self, val: Score) {
        self.insert_many(val, 1);
    }
    pub fn get_count(&self, val: &Score) -> u32 {
        *self.hist.get(&val).unwrap_or(&0)
    }
    pub fn percentage_with(&self, val: &Score) -> f32 {
        self.get_count(val) as f32 / self.total_count as f32
    }
    pub fn average(&self) -> f32 {
        (self.sum as f32) / (self.total_count as f32)
    }
    pub fn stdev_of_average(&self) -> f32 {
        let average = self.average();
        let mut var_sum = 0.0;
        for (&val, &count) in self.hist.iter() {
            var_sum += (val as f32 - average).powi(2) * count as f32;
        }
        // Divide by (self.total_count - 1) estimate the variance of the distribution,
        // then divide by self.total_count estimate the variance of the sample average,
        // then take the sqrt to get the stdev.
        (var_sum / (((self.total_count - 1) * self.total_count) as f32)).sqrt()
    }
    pub fn merge(&mut self, other: Histogram) {
        for (val, count) in other.hist.into_iter() {
            self.insert_many(val, count);
        }
    }
}
impl fmt::Display for Histogram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut keys = self.hist.keys().collect::<Vec<_>>();
        keys.sort();
        for val in keys {
            f.write_str(&format!(
                "\n{}: {}", val, self.get_count(val),
            ))?;
        }
        Ok(())
    }
}

pub fn simulate<T: ?Sized>(
        opts: &GameOptions,
        strat_config: Box<T>,
        first_seed_opt: Option<u32>,
        n_trials: u32,
        n_threads: u32,
        progress_info: Option<u32>,
    ) -> SimResult
    where T: GameStrategyConfig + Sync {

    let first_seed = first_seed_opt.unwrap_or_else(|| rand::thread_rng().next_u32());

    let strat_config_ref = &strat_config;
    crossbeam::scope(|scope| {
        let mut join_handles = Vec::new();
        for i in 0..n_threads {
            let start = first_seed + ((n_trials * i) / n_threads);
            let end = first_seed + ((n_trials * (i+1)) / n_threads);
            join_handles.push(scope.spawn(move || {
                if progress_info.is_some() {
                    info!("Thread {} spawned: seeds {} to {}", i, start, end);
                }
                let mut non_perfect_seeds = Vec::new();

                let mut score_histogram = Histogram::new();
                let mut lives_histogram = Histogram::new();

                for seed in start..end {
                    if let Some(progress_info_frequency) = progress_info {
                        if (seed > start) && ((seed-start) % progress_info_frequency == 0) {
                            info!(
                                "Thread {}, Trials: {}, Stats so far: {} score, {} lives, {}% win",
                                i, seed-start, score_histogram.average(), lives_histogram.average(),
                                score_histogram.percentage_with(&PERFECT_SCORE) * 100.0
                            );
                        }
                    }
                    let game = simulate_once(&opts, strat_config_ref.initialize(&opts), seed);
                    let score = game.score();
                    lives_histogram.insert(game.board.lives_remaining);
                    score_histogram.insert(score);
                    if score != PERFECT_SCORE { non_perfect_seeds.push(seed); }
                }
                if progress_info.is_some() {
                    info!("Thread {} done", i);
                }
                (non_perfect_seeds, score_histogram, lives_histogram)
            }));
        }

        let mut non_perfect_seeds : Vec<u32> = Vec::new();
        let mut score_histogram = Histogram::new();
        let mut lives_histogram = Histogram::new();
        for join_handle in join_handles {
            let (thread_non_perfect_seeds, thread_score_histogram, thread_lives_histogram) = join_handle.join();
            non_perfect_seeds.extend(thread_non_perfect_seeds.iter());
            score_histogram.merge(thread_score_histogram);
            lives_histogram.merge(thread_lives_histogram);
        }

        non_perfect_seeds.sort();
        SimResult {
            scores: score_histogram,
            lives: lives_histogram,
            non_perfect_seed: non_perfect_seeds.get(0).cloned(),
        }
    })
}

pub struct SimResult {
    pub scores: Histogram,
    pub lives: Histogram,
    pub non_perfect_seed: Option<u32>,
}

impl SimResult {
    pub fn percent_perfect(&self) -> f32 {
        self.scores.percentage_with(&PERFECT_SCORE) * 100.0
    }

    pub fn percent_perfect_stderr(&self) -> f32 {
        let pp = self.percent_perfect() / 100.0;
        let stdev = (pp*(1.0 - pp) / ((self.scores.total_count - 1) as f32)).sqrt();
        stdev * 100.0
    }

    pub fn average_score(&self) -> f32 {
        self.scores.average()
    }

    pub fn score_stderr(&self) -> f32 {
        self.scores.stdev_of_average()
    }

    pub fn average_lives(&self) -> f32 {
        self.lives.average()
    }

    pub fn info(&self) {
        info!("Score histogram:\n{}", self.scores);

        // info!("Seeds with non-perfect score: {:?}", non_perfect_seeds);
        if let Some(seed) = self.non_perfect_seed {
            info!("Example seed with non-perfect score: {}", seed);
        }

        info!("Percentage perfect: {:?}%", self.percent_perfect());
        info!("Average score: {:?}", self.average_score());
        info!("Average lives: {:?}", self.average_lives());
    }
}
