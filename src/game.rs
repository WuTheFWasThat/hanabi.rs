use fnv::FnvHashMap;
use std::fmt;
use std::ops::Range;

pub type Player = u32;

pub type Color = char;
pub const NUM_COLORS: usize = 5;
pub const COLORS: [Color; NUM_COLORS] = ['r', 'y', 'g', 'b', 'w'];

pub type Value = u32;
// list of values, assumed to be small to large
pub const NUM_VALUES: usize = 5;
pub const VALUES : [Value; NUM_VALUES] = [1, 2, 3, 4, 5];
pub const FINAL_VALUE : Value = 5;

pub fn get_count_for_value(value: Value) -> u32 {
    match value {
        1         => 3,
        2 | 3 | 4 => 2,
        5         => 1,
        _ => { panic!("Unexpected value: {}", value); }
    }
}

#[derive(Clone,PartialEq,Eq,Hash,Ord,PartialOrd)]
pub struct Card {
    pub color: Color,
    pub value: Value,
}
impl Card {
    pub fn new(color: Color, value: Value) -> Card {
        Card { color, value }
    }
}
impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.color, self.value)
    }
}
impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.color, self.value)
    }
}

#[derive(Debug,Clone,Eq,PartialEq)]
pub struct CardCounts {
    counts: FnvHashMap<Card, u32>,
}
impl CardCounts {
    pub fn new() -> CardCounts {
        let mut counts = FnvHashMap::default();
        for &color in COLORS.iter() {
            for &value in VALUES.iter() {
                counts.insert(Card::new(color, value), 0);
            }
        }
        CardCounts {
            counts,
        }
    }

    pub fn get_count(&self, card: &Card) -> u32 {
        *self.counts.get(card).unwrap()
    }

    pub fn remaining(&self, card: &Card) -> u32 {
        let count = self.get_count(card);
        get_count_for_value(card.value) - count
    }

    pub fn increment(&mut self, card: &Card) {
        let count = self.counts.get_mut(card).unwrap();
        *count += 1;
    }
}
impl fmt::Display for CardCounts {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for &color in COLORS.iter() {
            f.write_str(&format!(
                "{}: ", color,
            ))?;
            for &value in VALUES.iter() {
                let count = self.get_count(&Card::new(color, value));
                let total = get_count_for_value(value);
                f.write_str(&format!(
                    "{}/{} {}s", count, total, value
                ))?;
                if value != FINAL_VALUE {
                    f.write_str(", ")?;
                }
            }
            f.write_str("\n")?;
        }
        Ok(())
    }
}

pub type Cards = Vec<Card>;

#[derive(Debug,Clone,Eq,PartialEq)]
pub struct Discard {
    pub cards: Cards,
    counts: CardCounts,
}
impl Discard {
    pub fn new() -> Discard {
        Discard {
            cards: Cards::new(),
            counts: CardCounts::new(),
        }
    }

    pub fn has_all(&self, card: &Card) -> bool {
        self.counts.remaining(card) == 0
    }

    pub fn remaining(&self, card: &Card) -> u32 {
        self.counts.remaining(card)
    }

    pub fn place(&mut self, card: Card) {
        self.counts.increment(&card);
        self.cards.push(card);
    }
}
impl fmt::Display for Discard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // f.write_str(&format!(
        //     "{}", self.cards,
        // ))?;
        write!(f, "{}", self.counts)
    }
}

pub type Score = u32;
pub const PERFECT_SCORE: Score = (NUM_COLORS * NUM_VALUES) as u32;

#[derive(Debug,Clone,Eq,PartialEq)]
pub struct Firework {
    pub color: Color,
    pub top: Value,
}
impl Firework {
    pub fn new(color: Color) -> Firework {
        Firework {
            color,
            top: 0,
        }
    }

    pub fn needed_value(&self) -> Option<Value> {
        if self.complete() { None } else { Some(self.top + 1) }
    }

    pub fn score(&self) -> Score {
        self.top
    }

    pub fn complete(&self) -> bool {
        self.top == FINAL_VALUE
    }

    pub fn place(&mut self, card: &Card) {
        assert!(
            card.color == self.color,
            "Attempted to place card on firework of wrong color!"
        );
        assert!(
            Some(card.value) == self.needed_value(),
            "Attempted to place card of wrong value on firework!"
        );
        self.top = card.value;
    }
}
impl fmt::Display for Firework {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.complete() {
            write!(f, "{} firework complete!", self.color)
        } else {
            write!(f, "{} firework at {}", self.color, self.top)
        }
    }
}

#[derive(Debug,Clone,Hash,PartialEq,Eq)]
pub enum Hinted {
    Color(Color),
    Value(Value),
}
impl fmt::Display for Hinted {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Hinted::Color(color) => { write!(f, "{}", color) }
            Hinted::Value(value) => { write!(f, "{}", value) }
        }
    }
}

#[derive(Debug,Clone,Eq,PartialEq)]
pub struct Hint {
    pub player: Player,
    pub hinted: Hinted,
}

// represents the choice a player made in a given turn
#[derive(Debug,Clone,Eq,PartialEq)]
pub enum TurnChoice {
    Hint(Hint),
    Discard(usize), // index of card to discard
    Play(usize),    // index of card to play
}

// represents what happened in a turn
#[derive(Debug,Clone,Eq,PartialEq)]
pub enum TurnResult {
    Hint(Vec<bool>),  // vector of whether each was in the hint
    Discard(Card),    // card discarded
    Play(Card, bool), // card played, whether it succeeded
}

// represents a turn taken in the game
#[derive(Debug,Clone,Eq,PartialEq)]
pub struct TurnRecord {
    pub player: Player,
    pub choice: TurnChoice,
    pub result: TurnResult,
}
pub type TurnHistory = Vec<TurnRecord>;

// represents possible settings for the game
pub struct GameOptions {
    pub num_players: u32,
    pub hand_size: u32,
    // when hits 0, you cannot hint
    pub num_hints: u32,
    // when hits 0, you lose
    pub num_lives: u32,
    // whether to allow hints that reveal no cards
    pub allow_empty_hints: bool,
}

// State of everything except the player's hands
// Is all completely common knowledge
#[derive(Debug,Clone,Eq,PartialEq)]
pub struct BoardState {
    pub deck_size: u32,
    pub total_cards: u32,
    pub discard: Discard,
    pub fireworks: FnvHashMap<Color, Firework>,

    pub num_players: u32,

    // which turn is it?
    pub turn: u32,
    pub turn_history: TurnHistory,
    // // whose turn is it?
    pub player: Player,
    pub hand_size: u32,

    pub hints_total: u32,
    pub hints_remaining: u32,
    pub allow_empty_hints: bool,
    pub lives_total: u32,
    pub lives_remaining: u32,
    // only relevant when deck runs out
    pub deckless_turns_remaining: u32,
}
impl BoardState {
    pub fn new(opts: &GameOptions, deck_size: u32) -> BoardState {
        let fireworks = COLORS.iter().map(|&color| {
            (color, Firework::new(color))
        }).collect::<FnvHashMap<_, _>>();

        BoardState {
            deck_size,
            total_cards: deck_size,
            fireworks,
            discard: Discard::new(),
            num_players: opts.num_players,
            hand_size: opts.hand_size,
            player: 0,
            turn: 1,
            allow_empty_hints: opts.allow_empty_hints,
            hints_total: opts.num_hints,
            hints_remaining: opts.num_hints,
            lives_total: opts.num_lives,
            lives_remaining: opts.num_lives,
            turn_history: Vec::new(),
            // number of turns to play with deck length ran out
            deckless_turns_remaining: opts.num_players + 1,
        }
    }

    fn try_add_hint(&mut self) {
        if self.hints_remaining < self.hints_total {
            self.hints_remaining += 1;
        }
    }

    pub fn get_firework(&self, color: Color) -> &Firework {
        self.fireworks.get(&color).unwrap()
    }

    fn get_firework_mut(&mut self, color: Color) -> &mut Firework {
        self.fireworks.get_mut(&color).unwrap()
    }

    // returns whether a card would place on a firework
    pub fn is_playable(&self, card: &Card) -> bool {
        Some(card.value) == self.get_firework(card.color).needed_value()
    }

    // best possible value we can get for firework of that color,
    // based on looking at discard + fireworks
    fn highest_attainable(&self, color: Color) -> Value {
        let firework = self.fireworks.get(&color).unwrap();
        if firework.complete() {
            return FINAL_VALUE;
        }
        let needed = firework.needed_value().unwrap();

        for &value in VALUES.iter() {
            if value < needed {
                // already have these cards
                continue
            }
            let needed_card = Card::new(color, value);
            if self.discard.has_all(&needed_card) {
                // already discarded all of these
                return value - 1;
            }
        }
        FINAL_VALUE
    }

    // is never going to play, based on discard + fireworks
    pub fn is_dead(&self, card: &Card) -> bool {
        let firework = self.fireworks.get(&card.color).unwrap();
        if firework.complete() {
            true
        } else {
            let needed = firework.needed_value().unwrap();
            if card.value < needed {
                true
            } else {
                card.value > self.highest_attainable(card.color)
            }
        }
    }

    // can be discarded without necessarily sacrificing score, based on discard + fireworks
    pub fn is_dispensable(&self, card: &Card) -> bool {
        let firework = self.fireworks.get(&card.color).unwrap();
        if firework.complete() {
            true
        } else {
            let needed = firework.needed_value().unwrap();
            if card.value < needed || card.value > self.highest_attainable(card.color) {
                true
            } else {
                self.discard.remaining(card) != 1
            }
        }
    }

    pub fn get_players(&self) -> Range<Player> {
        0..self.num_players
    }

    pub fn score(&self) -> Score {
        self.fireworks.iter().map(|(_, firework)| firework.score()).sum()
    }

    pub fn discard_size(&self) -> u32 {
        self.discard.cards.len() as u32
    }

    pub fn player_to_left(&self, player: &Player) -> Player {
        (player + 1) % self.num_players
    }
    pub fn player_to_right(&self, player: &Player) -> Player {
        (player + self.num_players - 1) % self.num_players
    }

    pub fn is_over(&self) -> bool {
        (self.lives_remaining == 0) || (self.deckless_turns_remaining == 0)
    }
}
impl fmt::Display for BoardState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_over() {
            f.write_str(&format!(
                "Turn {} (GAME ENDED):\n", self.turn
            ))?;
        } else {
            f.write_str(&format!(
                "Turn {} (Player {}'s turn):\n", self.turn, self.player
            ))?;
        }

        f.write_str(&format!(
            "{} cards remaining in deck\n", self.deck_size
        ))?;
        if self.deck_size == 0 {
            f.write_str(&format!(
                "Deck is empty.  {} turns remaining in game\n", self.deckless_turns_remaining
            ))?;
        }
        f.write_str(&format!(
            "{}/{} hints remaining\n", self.hints_remaining, self.hints_total
        ))?;
        f.write_str(&format!(
            "{}/{} lives remaining\n", self.lives_remaining, self.lives_total
        ))?;
        f.write_str("Fireworks:\n")?;
        for &color in COLORS.iter() {
            f.write_str(&format!("  {}\n", self.get_firework(color)))?;
        }
        f.write_str("Discard:\n")?;
        f.write_str(&format!("{}\n", self.discard))?;

        Ok(())
    }
}

// complete game view of a given player
pub trait GameView {
    fn me(&self) -> Player;
    fn get_hand(&self, player: &Player) -> &Cards;
    fn get_board(&self) -> &BoardState;

    fn my_hand_size(&self) -> usize;

    fn hand_size(&self, player: &Player) -> usize {
        if self.me() == *player {
            self.my_hand_size()
        } else {
            self.get_hand(player).len()
        }
    }

    fn has_card(&self, player: &Player, card: &Card) -> bool {
        self.get_hand(player).iter().position(|other_card| {
            card == other_card
        }).is_some()
    }

    fn get_other_players(&self) -> Vec<Player> {
        self.get_board().get_players().filter(|&player| {
            player != self.me()
        }).collect()
    }

    fn can_see(&self, card: &Card) -> bool {
        self.get_other_players().iter().any(|player| {
            self.has_card(player, card)
        })
    }

    fn someone_else_can_play(&self) -> bool {
        self.get_other_players().iter().any(|player| {
            self.get_hand(player).iter().any(|card| {
                self.get_board().is_playable(card)
            })
        })
    }
}

// version of game view that is borrowed.  used in simulator for efficiency,
#[derive(Debug)]
pub struct BorrowedGameView<'a> {
    // the player whose view it is
    pub player: Player,
    pub hand_size: usize,
    // the cards of the other players, as well as the information they have
    pub other_hands: FnvHashMap<Player, &'a Cards>,
    // board state
    pub board: &'a BoardState,
}
impl <'a> GameView for BorrowedGameView<'a> {
    fn me(&self) -> Player {
        self.player
    }
    fn my_hand_size(&self) -> usize {
        self.hand_size
    }
    fn get_hand(&self, player: &Player) -> &Cards {
        assert!(self.me() != *player, "Cannot query about your own state!");
        self.other_hands.get(player).unwrap()
    }
    fn get_board(&self) -> &BoardState {
        self.board
    }
}

// version of game view, may be useful to strategies
#[derive(Debug)]
pub struct OwnedGameView {
    // the player whose view it is
    pub player: Player,
    pub hand_size: usize,
    // the cards of the other players, as well as the information they have
    pub other_hands: FnvHashMap<Player, Cards>,
    // board state
    pub board: BoardState,
}
impl OwnedGameView {
    pub fn clone_from(borrowed_view: &BorrowedGameView) -> OwnedGameView {
        let other_hands = borrowed_view.other_hands.iter()
            .map(|(&other_player, &player_state)| {
                (other_player, player_state.clone())
            }).collect::<FnvHashMap<_, _>>();

        OwnedGameView {
            player: borrowed_view.player,
            hand_size: borrowed_view.hand_size,
            other_hands,
            board: (*borrowed_view.board).clone(),
        }
    }
}
impl GameView for OwnedGameView {
    fn me(&self) -> Player {
        self.player
    }
    fn my_hand_size(&self) -> usize {
        self.hand_size
    }
    fn get_hand(&self, player: &Player) -> &Cards {
        assert!(self.me() != *player, "Cannot query about your own state!");
        self.other_hands.get(player).unwrap()
    }
    fn get_board(&self) -> &BoardState {
        &self.board
    }
}

// complete game state (known to nobody!)
#[derive(Debug)]
pub struct GameState {
    pub hands: FnvHashMap<Player, Cards>,
    pub board: BoardState,
    pub deck: Cards,
}
impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("\n")?;
        f.write_str("======\n")?;
        f.write_str("Hands:\n")?;
        f.write_str("======\n")?;
        for player in self.board.get_players() {
            let hand = &self.hands.get(&player).unwrap();
            f.write_str(&format!("player {}:", player))?;
            for card in hand.iter() {
                f.write_str(&format!("    {}", card))?;
            }
            f.write_str("\n")?;
        }
        f.write_str("======\n")?;
        f.write_str("Board:\n")?;
        f.write_str("======\n")?;
        f.write_str(&format!("{}", self.board))?;
        Ok(())
    }
}

impl GameState {
    pub fn new(opts: &GameOptions, mut deck: Cards) -> GameState {
        let mut board = BoardState::new(opts, deck.len() as u32);

        let hands =
            (0..opts.num_players).map(|player| {
                let hand = (0..opts.hand_size).map(|_| {
                    // we can assume the deck is big enough to draw initial hands
                    board.deck_size -= 1;
                    deck.pop().unwrap()
                }).collect::<Vec<_>>();
                (player, hand)
            }).collect::<FnvHashMap<_, _>>();

        GameState {
            hands,
            board,
            deck,
        }
    }

    pub fn get_players(&self) -> Range<Player> {
        self.board.get_players()
    }

    pub fn is_over(&self) -> bool {
        self.board.is_over()
    }

    pub fn score(&self) -> Score {
        self.board.score()
    }

    // get the game state view of a particular player
    pub fn get_view(&self, player: Player) -> BorrowedGameView {
        let mut other_hands = FnvHashMap::default();
        for (&other_player, hand) in &self.hands {
            if player != other_player {
                other_hands.insert(other_player, hand);
            }
        }
        BorrowedGameView {
            player,
            hand_size: self.hands.get(&player).unwrap().len(),
            other_hands,
            board: &self.board,
        }
    }

    // takes a card from the player's hand, and replaces it if possible
    fn take_from_hand(&mut self, index: usize) -> Card {
        let ref mut hand = self.hands.get_mut(&self.board.player).unwrap();
        hand.remove(index)
    }

    fn replenish_hand(&mut self) {
        let ref mut hand = self.hands.get_mut(&self.board.player).unwrap();
        if (hand.len() as u32) < self.board.hand_size {
            if let Some(new_card) = self.deck.pop() {
                self.board.deck_size -= 1;
                debug!("Drew new card, {}", new_card);
                hand.push(new_card);
            }
        }
    }

    pub fn process_choice(&mut self, choice: TurnChoice) -> TurnRecord {
        let turn_result = {
            match choice {
                TurnChoice::Hint(ref hint) => {
                    assert!(self.board.hints_remaining > 0,
                            "Tried to hint with no hints remaining");
                    self.board.hints_remaining -= 1;
                    debug!("Hint to player {}, about {}", hint.player, hint.hinted);

                    assert!(self.board.player != hint.player,
                            "Player {} gave a hint to himself", hint.player);

                    let hand = self.hands.get(&hint.player).unwrap();
                    let results = match hint.hinted {
                        Hinted::Color(color) => {
                            hand.iter().map(|card| { card.color == color }).collect::<Vec<_>>()
                        }
                        Hinted::Value(value) => {
                            hand.iter().map(|card| { card.value == value }).collect::<Vec<_>>()
                        }
                    };
                    if !self.board.allow_empty_hints {
                        assert!(results.iter().any(|matched| *matched),
                                "Tried hinting an empty hint");
                    }

                    TurnResult::Hint(results)
                }
                TurnChoice::Discard(index) => {
                    let card = self.take_from_hand(index);
                    debug!("Discard card in position {}, which is {}", index, card);
                    self.board.discard.place(card.clone());

                    self.board.try_add_hint();
                    TurnResult::Discard(card)
                }
                TurnChoice::Play(index) => {
                    let card = self.take_from_hand(index);

                    debug!(
                        "Playing card at position {}, which is {}",
                        index, card
                    );
                    let playable = self.board.is_playable(&card);
                    if playable {
                        {
                            let firework = self.board.get_firework_mut(card.color);
                            debug!("Successfully played {}!", card);
                            firework.place(&card);
                        }
                        if card.value == FINAL_VALUE {
                            debug!("Firework complete for {}!", card.color);
                            self.board.try_add_hint();
                        }
                    } else {
                        self.board.discard.place(card.clone());
                        self.board.lives_remaining -= 1;
                        debug!(
                            "Removing a life! Lives remaining: {}",
                            self.board.lives_remaining
                        );
                    }
                    TurnResult::Play(card, playable)
                }
            }
        };
        let turn_record = TurnRecord {
            player: self.board.player,
            result: turn_result,
            choice,
        };
        self.board.turn_history.push(turn_record.clone());

        self.replenish_hand();

        if self.board.deck_size == 0 {
            self.board.deckless_turns_remaining -= 1;
        }
        self.board.turn += 1;
        self.board.player = {
            let cur = self.board.player;
            self.board.player_to_left(&cur)
        };
        assert_eq!((self.board.turn - 1) % self.board.num_players, self.board.player);

        turn_record
    }
}
