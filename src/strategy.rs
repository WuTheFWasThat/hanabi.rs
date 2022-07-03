use game::*;

// Traits to implement for any valid Hanabi strategy

// Represents the strategy of a given player
pub trait PlayerStrategy {
    // A function to decide what to do on the player's turn.
    // Given a BorrowedGameView, outputs their choice.
    fn decide(&mut self, view: &BorrowedGameView) -> TurnChoice;
    // A function to update internal state after other players' turns.
    // Given what happened last turn, and the new state.
    fn update(&mut self, turn_record: &TurnRecord, view: &BorrowedGameView);
}
// Represents the overall strategy for a game
// Shouldn't do much, except store configuration parameters and
// possibility initialize some shared randomness between players
pub trait GameStrategy {
    fn initialize(&self, player: Player, view: &BorrowedGameView) -> Box<PlayerStrategy>;
}

// Represents configuration for a strategy.
// Acts as a factory for game strategies, so we can play many rounds
pub trait GameStrategyConfig {
    fn initialize(&self, opts: &GameOptions) -> Box<GameStrategy>;
}

