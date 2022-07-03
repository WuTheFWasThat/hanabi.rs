use std::rc::Rc;
use std::cell::{RefCell};
use fnv::{FnvHashMap, FnvHashSet};

use strategy::*;
use game::*;

// strategy that explicitly cheats by using Rc/RefCell
// serves as a reference point for other strategies
//
// Plays according to the following rules:
//  - if any card is playable,
//      play the card with the lowest value
//  - if a card is dead, discard it
//  - if another player has same card in hand, discard it
//  - if a card is discardable, discard it
//  - if a hint exists, hint
//  - discard the first card

pub struct CheatingStrategyConfig;

impl CheatingStrategyConfig {
    pub fn new() -> CheatingStrategyConfig {
        CheatingStrategyConfig
    }
}
impl GameStrategyConfig for CheatingStrategyConfig {
    fn initialize(&self, _: &GameOptions) -> Box<dyn GameStrategy> {
        Box::new(CheatingStrategy::new())
    }
}

pub struct CheatingStrategy {
    player_hands_cheat: Rc<RefCell<FnvHashMap<Player, Cards>>>,
}

impl CheatingStrategy {
    pub fn new() -> CheatingStrategy {
        CheatingStrategy {
            player_hands_cheat: Rc::new(RefCell::new(FnvHashMap::default())),
        }
    }
}
impl GameStrategy for CheatingStrategy {
    fn initialize(&self, player: Player, view: &BorrowedGameView) -> Box<dyn PlayerStrategy> {
        for (&player, &hand) in &view.other_hands {
            self.player_hands_cheat.borrow_mut().insert(
                player, hand.clone()
            );
        }
        Box::new(CheatingPlayerStrategy {
            player_hands_cheat: self.player_hands_cheat.clone(),
            me: player,
        })
    }
}

pub struct CheatingPlayerStrategy {
    player_hands_cheat: Rc<RefCell<FnvHashMap<Player, Cards>>>,
    me: Player,
}
impl CheatingPlayerStrategy {
    // last player might've drawn a new card, let him know!
    fn inform_last_player_cards(&self, view: &BorrowedGameView) {
        let next = view.board.player_to_right(&self.me);
        let their_hand = *view.other_hands.get(&next).unwrap();
        self.player_hands_cheat.borrow_mut().insert(
            next, their_hand.clone()
        );
    }

    // give a throwaway hint - we only do this when we have nothing to do
    fn throwaway_hint(&self, view: &BorrowedGameView) -> TurnChoice {
        let hint_player = view.board.player_to_left(&self.me);
        let hint_card = &view.get_hand(&hint_player).first().unwrap();
        TurnChoice::Hint(Hint {
            player: hint_player,
            hinted: Hinted::Value(hint_card.value)
        })
    }

    // represents how badly a card needs to be played
    fn card_play_value(&self, view: &BorrowedGameView, card: &Card) -> u32 {
        if view.board.is_dead(card) {
            return 0;
        }
        if !view.board.is_dispensable(card) {
            10 - card.value
        } else {
            1
        }
    }

    // given a hand of cards, represents how badly it will need to play things
    fn hand_play_value(&self, view: &BorrowedGameView, hand: &Cards) -> u32 {
        hand.iter().map(|card| self.card_play_value(view, card)).fold(0, |a,b| a+b)
    }

    // how badly do we need to play a particular card
    fn get_play_score(&self, view: &BorrowedGameView, card: &Card) -> i32 {
        let hands  = self.player_hands_cheat.borrow();
        let my_hand = hands.get(&self.me).unwrap();

        let my_hand_value = self.hand_play_value(view, my_hand);

        for player in view.board.get_players() {
            if player != self.me && view.has_card(&player, card) {
                let their_hand_value = self.hand_play_value(view, hands.get(&player).unwrap());
                // they can play this card, and have less urgent plays than i do
                if their_hand_value < my_hand_value {
                    return 10 - (card.value as i32)
                }
            }
        }
        // there are no hints
        // maybe value 5s more?
        20 - (card.value as i32)
    }

    fn find_useless_card(&self, view: &BorrowedGameView, hand: &Cards) -> Option<usize> {
        let mut set: FnvHashSet<Card> = FnvHashSet::default();

        for (i, card) in hand.iter().enumerate() {
            if view.board.is_dead(card) {
                return Some(i);
            }
            if set.contains(card) {
                // found a duplicate card
                return Some(i);
            }
            set.insert(card.clone());
        }
        return None
    }
}
impl PlayerStrategy for CheatingPlayerStrategy {
    fn decide(&mut self, view: &BorrowedGameView) -> TurnChoice {
        self.inform_last_player_cards(view);

        let hands = self.player_hands_cheat.borrow();
        let my_hand = hands.get(&self.me).unwrap();
        let playable_cards = my_hand.iter().enumerate().filter(|&(_, card)| {
            view.board.is_playable(card)
        }).collect::<Vec<_>>();

        if playable_cards.len() > 0 {
            // play the best playable card
            // the higher the play_score, the better to play
            let mut index = 0;
            let mut play_score = -1;

            for &(i, card) in playable_cards.iter() {
                let score = self.get_play_score(view, card);
                if score > play_score {
                    index = i;
                    play_score = score;
                }
            }
            return TurnChoice::Play(index)
        }

        // discard threshold is how many cards we're willing to discard
        // such that if we only played,
        // we would not reach the final countdown round
        // e.g. 50 total, 25 to play, 20 in hand
        let discard_threshold =
            view.board.total_cards
            - (COLORS.len() * VALUES.len()) as u32
            - (view.board.num_players * view.board.hand_size);
        if view.board.discard_size() <= discard_threshold {
            // if anything is totally useless, discard it
            if let Some(i) = self.find_useless_card(view, my_hand) {
                return TurnChoice::Discard(i);
            }
        }

        // hinting is better than discarding dead cards
        // (probably because it stalls the deck-drawing).
        if view.board.hints_remaining > 0 && view.someone_else_can_play() {
            return self.throwaway_hint(view);
        }

        // if anything is totally useless, discard it
        if let Some(i) = self.find_useless_card(view, my_hand) {
            return TurnChoice::Discard(i);
        }

        // All cards are plausibly useful.
        // Play the best discardable card, according to the ordering induced by comparing
        //   (is in another hand, is dispensable, value)
        // The higher, the better to discard
        let mut index = 0;
        let mut compval = (false, false, 0);
        for (i, card) in my_hand.iter().enumerate() {
            let my_compval = (
                view.can_see(card),
                view.board.is_dispensable(card),
                card.value,
            );
            if my_compval > compval {
                index = i;
                compval = my_compval;
            }
        }
        TurnChoice::Discard(index)
    }
    fn update(&mut self, _: &TurnRecord, _: &BorrowedGameView) {
    }
}
