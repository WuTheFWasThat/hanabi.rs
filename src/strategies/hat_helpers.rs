use game::*;
use helpers::*;

#[derive(Debug,Clone)]
pub struct ModulusInformation {
    pub modulus: u32,
    pub value: u32,
}
impl ModulusInformation {
    pub fn new(modulus: u32, value: u32) -> Self {
        assert!(value < modulus);
        ModulusInformation {
            modulus,
            value,
        }
    }

    pub fn none() -> Self {
        Self::new(1, 0)
    }

    pub fn combine(&mut self, other: Self, max_modulus: u32) {
        assert!(other.modulus <= self.info_remaining(max_modulus));
        self.value += self.modulus * other.value;
        self.modulus = std::cmp::min(max_modulus, self.modulus * other.modulus);
        assert!(self.value < self.modulus);
    }

    pub fn info_remaining(&self, max_modulus: u32) -> u32 {
        // We want to find the largest number `result` such that
        // `self.combine(other, max_modulus)` works whenever `other.modulus == result`.
        // `other.value` can be up to `result - 1`, so calling combine could increase our value to
        // up to `self.value + self.modulus * (result - 1)`, which must always be less than
        // `max_modulus`.
        // Therefore, we compute the largest number `result` such that
        // `self.value + self.modulus * (result - 1) < max_modulus`.
        let result = (max_modulus - self.value - 1) / self.modulus + 1;
        assert!(self.value + self.modulus * (result - 1) < max_modulus);
        assert!(self.value + self.modulus * ((result + 1) - 1) >= max_modulus);
        result
    }

    pub fn split(&mut self, modulus: u32) -> Self {
        assert!(self.modulus >= modulus);
        let original_modulus = self.modulus;
        let original_value = self.value;
        let value = self.value % modulus;
        self.value /= modulus;
        // `self.modulus` is the largest number such that
        // `value + (self.modulus - 1) * modulus < original_modulus`.
        // TODO: find an explanation of why this makes everything work out
        self.modulus = (original_modulus - value - 1) / modulus + 1;
        assert!(original_value == value + modulus * self.value);
        Self::new(modulus, value)
    }

    pub fn cast_up(&mut self, modulus: u32) {
        assert!(self.modulus <= modulus);
        self.modulus = modulus;
    }

    // pub fn cast_down(&mut self, modulus: u32) {
    //     assert!(self.modulus >= modulus);
    //     assert!(self.value < modulus);
    //     self.modulus = modulus;
    // }

    pub fn add(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.value + other.value) % self.modulus;
    }

    pub fn subtract(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.modulus + self.value - other.value) % self.modulus;
    }
}

pub trait Question {
    // how much info does this question ask for?
    fn info_amount(&self) -> u32;
    // get the answer to this question, given cards
    fn answer(&self, hand: &Cards, board: &BoardState) -> u32;
    // process the answer to this question, updating card info
    fn acknowledge_answer(
        &self, value: u32, hand_info: &mut HandInfo<CardPossibilityTable>, board: &BoardState
    );

    fn answer_info(&self, hand: &Cards, board: &BoardState) -> ModulusInformation {
        ModulusInformation::new(
            self.info_amount(),
            self.answer(hand, board)
        )
    }

    fn acknowledge_answer_info(
        &self,
        answer: ModulusInformation,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        board: &BoardState
    ) {
        assert!(self.info_amount() == answer.modulus);
        self.acknowledge_answer(answer.value, hand_info, board);
    }
}

pub trait PublicInformation: Clone {
    fn get_player_info(&self, player: &Player) -> HandInfo<CardPossibilityTable>;
    fn set_player_info(&mut self, player: &Player, hand_info: HandInfo<CardPossibilityTable>);

    fn new(board: &BoardState) -> Self;
    fn set_board(&mut self, board: &BoardState);

    /// If we store more state than just `HandInfo<CardPossibilityTable>`s, update it after `set_player_info` has been called.
    fn update_other_info(&mut self) {
    }

    fn agrees_with(&self, other: Self) -> bool;

    /// By defining `ask_question`, we decides which `Question`s a player learns the answers to.
    ///
    /// Whenever we need to compute a "hat value", this method will be called repeatedly, either
    /// until the information runs out, or until it returns `None`. These questions can depend on
    /// the answers to earlier questions: We are given a `&HandInfo<CardPossibilityTable>` that
    /// reflect the answers of previous questions for the same "hat value computation".
    ///
    /// Note that `self` does not reflect the answers to previous questions; it reflects the state
    /// before the entire "hat value" calculation.
    fn ask_question(&self, player: &Player, hand_info: &HandInfo<CardPossibilityTable>, total_info: u32) -> Option<Box<dyn Question>>;

    fn ask_question_wrapper(&self, player: &Player, hand_info: &HandInfo<CardPossibilityTable>, total_info: u32)
        -> Option<Box<dyn Question>>
    {
        assert!(total_info > 0);
        if total_info == 1 {
            None
        } else {
            let result = self.ask_question(player, hand_info, total_info);
            if let Some(ref question) = result {
                if question.info_amount() > total_info {
                    panic!("ask_question returned question with info_amount = {} > total_info = {}!",
                           question.info_amount(), total_info);
                }
                if question.info_amount() == 1 {
                    panic!("ask_question returned a trivial question!");
                }
            }
            result
        }
    }

    fn set_player_infos(&mut self, infos: Vec<(Player, HandInfo<CardPossibilityTable>)>) {
        for (player, new_hand_info) in infos {
            self.set_player_info(&player, new_hand_info);
        }
        self.update_other_info();
    }

    fn get_hat_info_for_player(
        &self, player: &Player, hand_info: &mut HandInfo<CardPossibilityTable>, total_info: u32, view: &OwnedGameView
    ) -> ModulusInformation {
        assert!(player != &view.player);
        let mut answer_info = ModulusInformation::none();
        while let Some(question) = self.ask_question_wrapper(player, hand_info, answer_info.info_remaining(total_info)) {
            let new_answer_info = question.answer_info(view.get_hand(player), view.get_board());
            question.acknowledge_answer_info(new_answer_info.clone(), hand_info, view.get_board());
            answer_info.combine(new_answer_info, total_info);
        }
        answer_info.cast_up(total_info);
        answer_info
    }

    fn update_from_hat_info_for_player(
        &self,
        player: &Player,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        board: &BoardState,
        mut info: ModulusInformation,
    ) {
        while let Some(question) = self.ask_question_wrapper(player, hand_info, info.modulus) {
            let answer_info = info.split(question.info_amount());
            question.acknowledge_answer_info(answer_info, hand_info, board);
        }
        assert!(info.value == 0);
    }

    /// When deciding on a move, if we can choose between `total_info` choices,
    /// `self.get_hat_sum(total_info, view)` tells us which choice to take, and at the same time
    /// mutates `self` to simulate the choice becoming common knowledge.
    fn get_hat_sum(&mut self, total_info: u32, view: &OwnedGameView) -> ModulusInformation {
        let (infos, new_player_hands): (Vec<_>, Vec<_>) = view.get_other_players().iter().map(|player| {
            let mut hand_info = self.get_player_info(player);
            let info = self.get_hat_info_for_player(player, &mut hand_info, total_info, view);
            (info, (*player, hand_info))
        }).unzip();
        self.set_player_infos(new_player_hands);
        infos.into_iter().fold(
            ModulusInformation::new(total_info, 0),
            |mut sum_info, info| {
                sum_info.add(&info);
                sum_info
            }
        )
    }

    /// When updating on a move, if we infer that the player making the move called `get_hat_sum()`
    /// and got the result `info`, we can call `self.update_from_hat_sum(info, view)` to update
    /// from that fact.
    fn update_from_hat_sum(&mut self, mut info: ModulusInformation, view: &OwnedGameView) {
        let info_source = view.board.player;
        let (other_infos, mut new_player_hands): (Vec<_>, Vec<_>) = view.get_other_players().into_iter().filter(|player| {
            *player != info_source
        }).map(|player| {
            let mut hand_info = self.get_player_info(&player);
            let player_info = self.get_hat_info_for_player(&player, &mut hand_info, info.modulus, view);
            (player_info, (player, hand_info))
        }).unzip();
        for other_info in other_infos {
            info.subtract(&other_info);
        }
        let me = view.player;
        if me == info_source {
            assert!(info.value == 0);
        } else {
            let mut my_hand = self.get_player_info(&me);
            self.update_from_hat_info_for_player(&me, &mut my_hand, &view.board, info);
            new_player_hands.push((me, my_hand));
        }
        self.set_player_infos(new_player_hands);
    }

    fn get_private_info(&self, view: &OwnedGameView) -> HandInfo<CardPossibilityTable> {
        let mut info = self.get_player_info(&view.player);
        for card_table in info.iter_mut() {
            for (_, hand) in &view.other_hands {
                for card in hand {
                    card_table.decrement_weight_if_possible(card);
                }
            }
        }
        info
    }
}
