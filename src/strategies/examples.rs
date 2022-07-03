use strategy::*;
use game::*;
use rand::{self, Rng};

// dummy, terrible strategy, as an example
#[derive(Clone)]
pub struct RandomStrategyConfig {
    pub hint_probability: f64,
    pub play_probability: f64,
}

impl GameStrategyConfig for RandomStrategyConfig {
    fn initialize(&self, _: &GameOptions) -> Box<dyn GameStrategy> {
        Box::new(RandomStrategy {
            hint_probability: self.hint_probability,
            play_probability: self.play_probability,
        })
    }
}

pub struct RandomStrategy {
    hint_probability: f64,
    play_probability: f64,
}
impl GameStrategy for RandomStrategy {
    fn initialize(&self, player: Player, _: &BorrowedGameView) -> Box<dyn PlayerStrategy> {
        Box::new(RandomStrategyPlayer {
            hint_probability: self.hint_probability,
            play_probability: self.play_probability,
            me: player,
        })
    }
}

pub struct RandomStrategyPlayer {
    hint_probability: f64,
    play_probability: f64,
    me: Player,
}

impl PlayerStrategy for RandomStrategyPlayer {
    fn decide(&mut self, view: &BorrowedGameView) -> TurnChoice {
        let p = rand::random::<f64>();
        if p < self.hint_probability {
            if view.board.hints_remaining > 0 {
                let hint_player = view.board.player_to_left(&self.me);
                let hint_card = rand::thread_rng().choose(view.get_hand(&hint_player)).unwrap();
                let hinted = {
                    if rand::random() {
                        // hint a color
                        Hinted::Color(hint_card.color)
                    } else {
                        Hinted::Value(hint_card.value)
                    }
                };
                TurnChoice::Hint(Hint {
                    player: hint_player,
                    hinted,
                })
            } else {
                TurnChoice::Discard(0)
            }
        } else if p < self.hint_probability + self.play_probability {
            TurnChoice::Play(0)
        } else {
            TurnChoice::Discard(0)
        }
    }
    fn update(&mut self, _: &TurnRecord, _: &BorrowedGameView) {
    }
}
