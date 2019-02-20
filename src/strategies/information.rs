use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;

use strategy::*;
use game::*;
use helpers::*;

// TODO: use random extra information - i.e. when casting up and down,
// we sometimes have 2 choices of value to choose
// TODO: guess very aggressively at very end of game (first, see whether
// situation ever occurs)

#[derive(Debug,Clone)]
struct ModulusInformation {
    modulus: u32,
    value: u32,
}
impl ModulusInformation {
    pub fn new(modulus: u32, value: u32) -> Self {
        assert!(value < modulus);
        ModulusInformation {
            modulus: modulus,
            value: value,
        }
    }

    pub fn none() -> Self {
        Self::new(1, 0)
    }

    pub fn combine(&mut self, other: Self) {
        self.value = self.value * other.modulus + other.value;
        self.modulus = self.modulus * other.modulus;
    }

    pub fn split(&mut self, modulus: u32) -> Self {
        assert!(self.modulus >= modulus);
        assert!(self.modulus % modulus == 0);
        let original_modulus = self.modulus;
        let original_value = self.value;
        self.modulus = self.modulus / modulus;
        let value = self.value / self.modulus;
        self.value = self.value - value * self.modulus;
        assert!(original_modulus == modulus * self.modulus);
        assert!(original_value == value * self.modulus + self.value);
        Self::new(modulus, value)
    }

    pub fn cast_up(&mut self, modulus: u32) {
        assert!(self.modulus <= modulus);
        self.modulus = modulus;
    }

    pub fn cast_down(&mut self, modulus: u32) {
        assert!(self.modulus >= modulus);
        assert!(self.value < modulus);
        self.modulus = modulus;
    }

    pub fn add(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.value + other.value) % self.modulus;
    }

    pub fn subtract(&mut self, other: &Self) {
        assert!(self.modulus == other.modulus);
        self.value = (self.modulus + self.value - other.value) % self.modulus;
    }
}

trait Question {
    // how much info does this question ask for?
    fn info_amount(&self) -> u32;
    // get the answer to this question, given cards
    fn answer(&self, &Cards, &OwnedGameView) -> u32;
    // process the answer to this question, updating card info
    fn acknowledge_answer(
        &self, value: u32, &mut HandInfo<CardPossibilityTable>, &OwnedGameView
    );

    fn answer_info(&self, hand: &Cards, view: &OwnedGameView) -> ModulusInformation {
        ModulusInformation::new(
            self.info_amount(),
            self.answer(hand, view)
        )
    }

    fn acknowledge_answer_info(
        &self,
        answer: ModulusInformation,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        view: &OwnedGameView
    ) {
        assert!(self.info_amount() == answer.modulus);
        self.acknowledge_answer(answer.value, hand_info, view);
    }
}

type PropertyPredicate = fn(&BoardState, &Card) -> bool;
struct CardHasProperty
{
    index: usize,
    property: PropertyPredicate,
}
impl Question for CardHasProperty
{
    fn info_amount(&self) -> u32 { 2 }
    fn answer(&self, hand: &Cards, view: &OwnedGameView) -> u32 {
        let ref card = hand[self.index];
        if (self.property)(view.get_board(), card) { 1 } else { 0 }
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        view: &OwnedGameView,
    ) {
        let ref mut card_table = hand_info[self.index];
        let possible = card_table.get_possibilities();
        for card in &possible {
            if (self.property)(view.get_board(), card) {
                if answer == 0 { card_table.mark_false(card); }
            } else {
                if answer == 1 { card_table.mark_false(card); }
            }
        }
    }
}
fn q_is_playable(index: usize) -> CardHasProperty {
    CardHasProperty {index, property: |board, card| board.is_playable(card)}
}
fn q_is_dead(index: usize) -> CardHasProperty {
    CardHasProperty {index, property: |board, card| board.is_dead(card)}
}

struct AdditiveComboQuestion {
    /// For some list of questions l, the question `AdditiveComboQuestion { questions : l }` asks:
    /// "What is the first question in the list `l` that has a nonzero answer, and what is its
    /// answer?"
    /// If all questions in `l` have the answer `0`, this question has the answer `0` as well.
    ///
    /// It's named that way because the `info_amount` grows additively with the `info_amount`s of
    /// the questions in `l`.
    questions: Vec<Box<Question>>,
}
impl Question for AdditiveComboQuestion {
    fn info_amount(&self) -> u32 {
        self.questions.iter().map(|q| { q.info_amount() - 1 }).sum::<u32>() + 1
    }
    fn answer(&self, hand: &Cards, view: &OwnedGameView) -> u32 {
        let mut toadd = 1;
        for q in &self.questions {
            let q_answer = q.answer(hand, view);
            if q_answer != 0 {
                return toadd + q_answer - 1;
            }
            toadd += q.info_amount() - 1;
        }
        assert!(toadd == self.info_amount());
        0
    }
    fn acknowledge_answer(
        &self,
        mut answer: u32,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        view: &OwnedGameView,
    ) {
        if answer == 0 {
            answer = self.info_amount();
        }
        answer -= 1;
        for q in &self.questions {
            if answer < q.info_amount() - 1 {
                q.acknowledge_answer(answer+1, hand_info, view);
                return;
            } else {
                q.acknowledge_answer(0, hand_info, view);
                answer -= q.info_amount() - 1;
            }
        }
        assert!(answer == 0);
    }
}

struct CardPossibilityPartition {
    index: usize,
    n_partitions: u32,
    partition: HashMap<Card, u32>,
}
impl CardPossibilityPartition {
    fn new(
        index: usize, max_n_partitions: u32, card_table: &CardPossibilityTable, view: &OwnedGameView
    ) -> CardPossibilityPartition {
        let mut cur_block = 0;
        let mut partition = HashMap::new();
        let mut n_partitions = 0;

        let has_dead = card_table.probability_is_dead(&view.board) != 0.0;

        // TODO: group things of different colors and values?
        let mut effective_max = max_n_partitions;
        if has_dead {
            effective_max -= 1;
        };

        for card in card_table.get_possibilities() {
            if !view.board.is_dead(&card) {
                partition.insert(card.clone(), cur_block);
                cur_block = (cur_block + 1) % effective_max;
                if n_partitions < effective_max {
                    n_partitions += 1;
                }
            }
        }

        if has_dead {
            for card in card_table.get_possibilities() {
                if view.board.is_dead(&card) {
                    partition.insert(card.clone(), n_partitions);
                }
            }
            n_partitions += 1;
        }

        // let mut s : String = "Partition: |".to_string();
        // for i in 0..n_partitions {
        //     for (card, block) in partition.iter() {
        //         if *block == i {
        //             s = s + &format!(" {}", card);
        //         }
        //     }
        //     s = s + &format!(" |");
        // }
        // debug!("{}", s);

        CardPossibilityPartition {
            index: index,
            n_partitions: n_partitions,
            partition: partition,
        }
    }
}
impl Question for CardPossibilityPartition {
    fn info_amount(&self) -> u32 { self.n_partitions }
    fn answer(&self, hand: &Cards, _: &OwnedGameView) -> u32 {
        let ref card = hand[self.index];
        *self.partition.get(&card).unwrap()
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        _: &OwnedGameView,
    ) {
        let ref mut card_table = hand_info[self.index];
        let possible = card_table.get_possibilities();
        for card in &possible {
            if *self.partition.get(card).unwrap() != answer {
                card_table.mark_false(card);
            }
        }
    }
}

pub struct InformationStrategyConfig;

impl InformationStrategyConfig {
    pub fn new() -> InformationStrategyConfig {
        InformationStrategyConfig
    }
}
impl GameStrategyConfig for InformationStrategyConfig {
    fn initialize(&self, _: &GameOptions) -> Box<GameStrategy> {
        Box::new(InformationStrategy::new())
    }
}

pub struct InformationStrategy;

impl InformationStrategy {
    pub fn new() -> InformationStrategy {
        InformationStrategy
    }
}
impl GameStrategy for InformationStrategy {
    fn initialize(&self, player: Player, view: &BorrowedGameView) -> Box<PlayerStrategy> {
        let public_info =
            view.board.get_players().map(|player| {
                let hand_info = HandInfo::new(view.board.hand_size);
                (player, hand_info)
            }).collect::<HashMap<_,_>>();

        Box::new(InformationPlayerStrategy {
            me: player,
            public_info: public_info,
            public_counts: CardCounts::new(),
            last_view: OwnedGameView::clone_from(view),
        })
    }
}

pub struct InformationPlayerStrategy {
    me: Player,
    public_info: HashMap<Player, HandInfo<CardPossibilityTable>>,
    public_counts: CardCounts, // what any newly drawn card should be
    last_view: OwnedGameView, // the view on the previous turn
}

impl InformationPlayerStrategy {

    fn get_questions(
        total_info: u32,
        view: &OwnedGameView,
        hand_info: &HandInfo<CardPossibilityTable>,
    ) -> Vec<Box<Question>> {
        let mut questions = Vec::new();
        let mut info_remaining = total_info;

        fn add_question<T: 'static>(
            questions: &mut Vec<Box<Question>>, info_remaining: &mut u32, question: T
        ) -> bool where T: Question {
            *info_remaining = *info_remaining / question.info_amount();
            questions.push(Box::new(question) as Box<Question>);

            // if there's no more info to give, return that we should stop
            *info_remaining <= 1
        }

        let augmented_hand_info = hand_info.iter().enumerate()
            .map(|(i, card_table)| {
                let p_play = card_table.probability_is_playable(&view.board);
                let p_dead = card_table.probability_is_dead(&view.board);
                let is_determined = card_table.is_determined();
                (card_table, i, p_play, p_dead, is_determined)
            })
            .collect::<Vec<_>>();

        let known_playable = augmented_hand_info.iter().filter(|&&(_, _, p_play, _, _)| {
            p_play == 1.0
        }).collect::<Vec<_>>().len();
        let known_dead = augmented_hand_info.iter().filter(|&&(_, _, _, p_dead, _)| {
            p_dead == 1.0
        }).collect::<Vec<_>>().len();

        if known_playable == 0 { // TODO: changing this to "if true {" slightly improves the three-player game and
                                 // very slightly worsens the other cases. There probably is some
                                 // other way to make this decision that's better in all cases.
            let mut ask_play = augmented_hand_info.iter()
                .filter(|&&(_, _, p_play, p_dead, is_determined)| {
                    if is_determined { return false; }
                    if p_dead == 1.0  { return false; }
                    if p_play == 1.0 || p_play < 0.2 { return false; }
                    true
                }).collect::<Vec<_>>();
            // sort by probability of play, then by index
            ask_play.sort_by(|&&(_, i1, p1, _, _), &&(_, i2, p2, _, _)| {
                    // It's better to include higher-probability-of-playability
                    // cards into our combo question, since that maximizes our
                    // chance of finding out about a playable card.
                    let result = p2.partial_cmp(&p1);
                    if result == None || result == Some(Ordering::Equal) {
                        i1.cmp(&i2)
                    } else {
                        result.unwrap()
                    }
                });

            if view.board.num_players == 5 {
                for &(_, i, _, _, _) in ask_play {
                    if add_question(&mut questions, &mut info_remaining, q_is_playable(i)) {
                        return questions;
                    }
                }
            } else {
                let mut rest_combo = AdditiveComboQuestion {questions: Vec::new()};
                for &(_, i, _, _, _) in ask_play {
                    if rest_combo.info_amount() < info_remaining {
                        rest_combo.questions.push(Box::new(q_is_playable(i)));
                    }
                }
                rest_combo.questions.reverse(); // It's better to put lower-probability-of-playability
                                                // cards first: The difference only matters if we
                                                // find a playable card, and conditional on that,
                                                // it's better to find out about as many non-playable
                                                // cards as possible.
                if rest_combo.info_amount() < info_remaining && known_dead == 0 {
                    let mut ask_dead = augmented_hand_info.iter()
                        .filter(|&&(_, _, _, p_dead, _)| {
                            p_dead > 0.0 && p_dead < 1.0
                        }).collect::<Vec<_>>();
                    // sort by probability of death, then by index
                    ask_dead.sort_by(|&&(_, i1, _, d1, _), &&(_, i2, _, d2, _)| {
                            let result = d2.partial_cmp(&d1);
                            if result == None || result == Some(Ordering::Equal) {
                                i1.cmp(&i2)
                            } else {
                                result.unwrap()
                            }
                        });
                    for &(_, i, _, _, _) in ask_dead {
                        if rest_combo.info_amount() < info_remaining {
                            rest_combo.questions.push(Box::new(q_is_dead(i)));
                        }
                    }
                }
                if add_question(&mut questions, &mut info_remaining, rest_combo) {
                    return questions;
                }
            }
        }

        let mut ask_partition = augmented_hand_info.iter()
            .filter(|&&(_, _, _, p_dead, is_determined)| {
                if is_determined { return false }
                // TODO: possibly still valuable to ask?
                if p_dead == 1.0 { return false }
                true
            }).collect::<Vec<_>>();
        // sort by probability of play, then by index
        ask_partition.sort_by(|&&(_, i1, p1, _, _), &&(_, i2, p2, _, _)| {
                // *higher* probabilities are better
                let result = p2.partial_cmp(&p1);
                if result == None || result == Some(Ordering::Equal) {
                    i1.cmp(&i2)
                } else {
                    result.unwrap()
                }
            });

        for &(card_table, i, _, _, _) in ask_partition {
            let question = CardPossibilityPartition::new(i, info_remaining, card_table, view);
            if add_question(&mut questions, &mut info_remaining, question) {
                return questions;
            }
        }

        return questions
    }

    fn answer_questions(
        questions: &Vec<Box<Question>>, hand: &Cards, view: &OwnedGameView
    ) -> ModulusInformation {
        questions.iter()
            .fold(
                ModulusInformation::none(),
                |mut answer_info, question| {
                  let new_answer_info = question.answer_info(hand, view);
                  answer_info.combine(new_answer_info);
                  answer_info
                })
    }

    fn get_hint_info_for_player(
        &self, player: &Player, total_info: u32, view: &OwnedGameView
    ) -> ModulusInformation {
        assert!(player != &self.me);
        let hand_info = self.get_player_public_info(player);
        let questions = Self::get_questions(total_info, view, hand_info);
        let mut answer = Self::answer_questions(&questions, view.get_hand(player), view);
        answer.cast_up(total_info);
        answer
    }

    fn get_hint_sum_info(&self, total_info: u32, view: &OwnedGameView) -> ModulusInformation {
        view.get_board().get_players().filter(|&player| {
            player != self.me
        }).fold(
            ModulusInformation::new(total_info, 0),
            |mut sum_info, player| {
                let answer = self.get_hint_info_for_player(&player, total_info, view);
                sum_info.add(&answer);
                sum_info
        })
    }

    fn infer_own_from_hint_sum(&mut self, mut hint: ModulusInformation) {
        let questions = {
            let view = &self.last_view;
            let hand_info = self.get_my_public_info();
            Self::get_questions(hint.modulus, view, hand_info)
        };
        let product = questions.iter().fold(1, |a, ref b| a * b.info_amount());
        hint.cast_down(product);

        let me = self.me.clone();
        let mut hand_info = self.take_public_info(&me);

        {
            let view = &self.last_view;
            for question in questions {
                let answer_info = hint.split(question.info_amount());
                question.acknowledge_answer_info(answer_info, &mut hand_info, view);
            }
        }
        self.return_public_info(&me, hand_info);
    }

    fn update_from_hint_sum(&mut self, mut hint: ModulusInformation) {
        let hinter = self.last_view.board.player;
        let players = {
            let view = &self.last_view;
            view.board.get_players()
        };
        for player in players {
            if (player != hinter) && (player != self.me) {
                {
                    let view = &self.last_view;
                    let hint_info = self.get_hint_info_for_player(&player, hint.modulus, view);
                    hint.subtract(&hint_info);
                }

                // *take* instead of borrowing mutably, because of borrow rules...
                let mut hand_info = self.take_public_info(&player);

                {
                    let view = &self.last_view;
                    let hand = view.get_hand(&player);
                    let questions = Self::get_questions(hint.modulus, view, &mut hand_info);
                    for question in questions {
                        let answer = question.answer(hand, view);
                        question.acknowledge_answer(answer, &mut hand_info, view);
                    }
                }
                self.return_public_info(&player, hand_info);
            }
        }
        if self.me == hinter {
            assert!(hint.value == 0);
        } else {
            self.infer_own_from_hint_sum(hint);
        }
    }

    // how badly do we need to play a particular card
    fn get_average_play_score(&self, view: &OwnedGameView, card_table: &CardPossibilityTable) -> f32 {
        let f = |card: &Card| { self.get_play_score(view, card) };
        card_table.weighted_score(&f)
    }

    fn get_play_score(&self, view: &OwnedGameView, card: &Card) -> f32 {
        let mut num_with = 1;
        if view.board.deck_size > 0 {
            for player in view.board.get_players() {
                if player != self.me {
                    if view.has_card(&player, card) {
                        num_with += 1;
                    }
                }
            }
        }
        (10.0 - card.value as f32) / (num_with as f32)
    }

    fn find_useless_cards(&self, view: &OwnedGameView, hand: &HandInfo<CardPossibilityTable>) -> Vec<usize> {
        let mut useless: HashSet<usize> = HashSet::new();
        let mut seen: HashMap<Card, usize> = HashMap::new();

        for (i, card_table) in hand.iter().enumerate() {
            if card_table.probability_is_dead(view.get_board()) == 1.0 {
                useless.insert(i);
            } else {
                if let Some(card) = card_table.get_card() {
                    if seen.contains_key(&card) {
                        // found a duplicate card
                        useless.insert(i);
                        useless.insert(*seen.get(&card).unwrap());
                    } else {
                        seen.insert(card, i);
                    }
                }
            }
        }
        let mut useless_vec : Vec<usize> = useless.into_iter().collect();
        useless_vec.sort();
        return useless_vec;
    }

    fn take_public_info(&mut self, player: &Player) -> HandInfo<CardPossibilityTable> {
        self.public_info.remove(player).unwrap()
    }

    fn return_public_info(&mut self, player: &Player, card_info: HandInfo<CardPossibilityTable>) {
        self.public_info.insert(*player, card_info);
    }

    fn get_my_public_info(&self) -> &HandInfo<CardPossibilityTable> {
        self.get_player_public_info(&self.me)
    }

    // fn get_my_public_info_mut(&mut self) -> &mut HandInfo<CardPossibilityTable> {
    //     let me = self.me.clone();
    //     self.get_player_public_info_mut(&me)
    // }

    fn get_player_public_info(&self, player: &Player) -> &HandInfo<CardPossibilityTable> {
        self.public_info.get(player).unwrap()
    }

    fn get_player_public_info_mut(&mut self, player: &Player) -> &mut HandInfo<CardPossibilityTable> {
        self.public_info.get_mut(player).unwrap()
    }

    fn update_public_info_for_hint(&mut self, hint: &Hint, matches: &Vec<bool>) {
        let info = self.get_player_public_info_mut(&hint.player);
        info.update_for_hint(&hint.hinted, matches);
    }

    fn update_public_info_for_discard_or_play(
        &mut self,
        view: &BorrowedGameView,
        player: &Player,
        index: usize,
        card: &Card
    ) {
        let new_card_table = CardPossibilityTable::from(&self.public_counts);
        {
            let info = self.get_player_public_info_mut(&player);
            assert!(info[index].is_possible(card));
            info.remove(index);

            // push *before* incrementing public counts
            if info.len() < view.hand_size(&player) {
                info.push(new_card_table);
            }
        }

        // TODO: decrement weight counts for fully determined cards, ahead of time

        // note: other_player could be player, as well
        // in particular, we will decrement the newly drawn card
        for other_player in view.board.get_players() {
            let info = self.get_player_public_info_mut(&other_player);
            for card_table in info.iter_mut() {
                card_table.decrement_weight_if_possible(card);
            }
        }

        self.public_counts.increment(card);
    }

    fn get_private_info(&self, view: &OwnedGameView) -> HandInfo<CardPossibilityTable> {
        let mut info = self.get_my_public_info().clone();
        for card_table in info.iter_mut() {
            for (_, hand) in &view.other_hands {
                for card in hand {
                    card_table.decrement_weight_if_possible(card);
                }
            }
        }
        info
    }

    fn get_hint_index_score(&self, card_table: &CardPossibilityTable, view: &OwnedGameView) -> i32 {
        if card_table.probability_is_dead(view.get_board()) == 1.0 {
            return 0;
        }
        if card_table.is_determined() {
            return 0;
        }
        // Do something more intelligent?
        let mut score = 1;
        if !card_table.color_determined() {
            score += 1;
        }
        if !card_table.value_determined() {
            score += 1;
        }
        return score;
    }

    fn get_index_for_hint(&self, info: &HandInfo<CardPossibilityTable>, view: &OwnedGameView) -> usize {
        let mut scores = info.iter().enumerate().map(|(i, card_table)| {
            let score = self.get_hint_index_score(card_table, view);
            (-score, i)
        }).collect::<Vec<_>>();
        scores.sort();
        scores[0].1
    }

    // how good is it to give this hint to this player?
    fn hint_goodness(&self, hinted: &Hinted, hint_player: &Player, view: &OwnedGameView) -> f32 {
        let hand = view.get_hand(&hint_player);

        // get post-hint hand_info
        let mut hand_info = self.get_player_public_info(hint_player).clone();
        let total_info  = 3 * (view.board.num_players - 1);
        let questions = Self::get_questions(total_info, view, &hand_info);
        for question in questions {
            let answer = question.answer(hand, view);
            question.acknowledge_answer(answer, &mut hand_info, view);
        }

        let mut goodness = 1.0;
        for (i, card_table) in hand_info.iter_mut().enumerate() {
            let card = &hand[i];
            if card_table.probability_is_dead(&view.board) == 1.0 {
                continue;
            }
            if card_table.is_determined() {
                continue;
            }
            let old_weight = card_table.total_weight();
            match *hinted {
                Hinted::Color(color) => {
                    card_table.mark_color(color, color == card.color)
                }
                Hinted::Value(value) => {
                    card_table.mark_value(value, value == card.value)
                }
            };
            let new_weight = card_table.total_weight();
            assert!(new_weight <= old_weight);
            let bonus = {
                if card_table.is_determined() {
                    2
                } else if card_table.probability_is_dead(&view.board) == 1.0 {
                    2
                } else {
                    1
                }
            };
            goodness *= (bonus as f32) * (old_weight / new_weight);
        }
        goodness
    }

    // Returns the number of ways to hint the player.
    fn get_info_per_player(&self, player: Player) -> u32 {
        let info = self.get_player_public_info(&player);

        let may_be_all_one_color = COLORS.iter().any(|color| {
            info.iter().all(|card| {
                card.can_be_color(*color)
            })
        });

        let may_be_all_one_number = VALUES.iter().any(|value| {
            info.iter().all(|card| {
                card.can_be_value(*value)
            })
        });

        return if !may_be_all_one_color && !may_be_all_one_number { 4 } else { 3 }

        // Determine if both:
        //  - it is public that there are at least two colors
        //  - it is public that there are at least two numbers
    }

    fn get_other_players_starting_after(&self, player: Player) -> Vec<Player> {
        let view = &self.last_view;
        let n = view.board.num_players;
        (0 .. n - 1).into_iter().map(|i| { (player + 1 + i) % n }).collect()
    }

    fn get_best_hint_of_options(&self, hint_player: Player, hint_option_set: HashSet<Hinted>) -> Hinted {
        let view = &self.last_view;

        // using hint goodness barely helps
        let mut hint_options = hint_option_set.into_iter().map(|hinted| {
            (self.hint_goodness(&hinted, &hint_player, view), hinted)
        }).collect::<Vec<_>>();

        hint_options.sort_by(|h1, h2| {
            h2.0.partial_cmp(&h1.0).unwrap_or(Ordering::Equal)
        });

        if hint_options.len() == 0 {
            // NOTE: Technically possible, but never happens
        } else {
            if hint_options.len() > 1 {
                debug!("Choosing amongst hint options: {:?}", hint_options);
            }
        }
        hint_options.remove(0).1
    }

    fn get_hint(&self) -> TurnChoice {
        let view = &self.last_view;

        // Can give up to 3(n-1) hints
        // For any given player with at least 4 cards, and index i, there are at least 3 hints that can be given.
        // 0. a value hint on card i
        // 1. a color hint on card i
        // 2. any hint not involving card i
        // However, if it is public info that the player has at least two colors
        // and at least two numbers, then instead we do
        // 2. any color hint not involving i
        // 3. any color hint not involving i

        // TODO: make it so space of hints is larger when there is
        // knowledge about the cards?

        let info_per_player: Vec<Player> = self.get_other_players_starting_after(self.me).iter().map(
            |player| { self.get_info_per_player(*player)  }
        ).collect();
        let total_info = info_per_player.iter().fold(0, |a, b| a + b);

        let hint_info = self.get_hint_sum_info(total_info, view);

        //let hint_type = hint_info.value % 3;
        //let player_amt = (hint_info.value - hint_type) / 3;
        let mut hint_type = hint_info.value;
        let mut player_amt = 0;
        while hint_type >= info_per_player[player_amt] {
            hint_type -= info_per_player[player_amt];
            player_amt += 1;
        }
        let hint_info_we_can_give_to_this_player = info_per_player[player_amt];

        let hint_player = (self.me + 1 + (player_amt as u32)) % view.board.num_players;

        let hand = view.get_hand(&hint_player);
        let card_index = self.get_index_for_hint(self.get_player_public_info(&hint_player), view);
        let hint_card = &hand[card_index];

        let hinted = if hint_info_we_can_give_to_this_player == 3 {
            match hint_type {
                0 => {
                    Hinted::Value(hint_card.value)
                }
                1 => {
                    Hinted::Color(hint_card.color)
                }
                2 => {
                    // NOTE: this doesn't do that much better than just hinting
                    // the first thing that doesn't match the hint_card
                    let mut hint_option_set = HashSet::new();
                    for card in hand {
                        if card.color != hint_card.color {
                            hint_option_set.insert(Hinted::Color(card.color));
                        }
                        if card.value != hint_card.value {
                            hint_option_set.insert(Hinted::Value(card.value));
                        }
                    }
                    self.get_best_hint_of_options(hint_player, hint_option_set)
                }
                _ => {
                    panic!("Invalid hint type")
                }
            }
        } else {
            match hint_type {
                0 => {
                    Hinted::Value(hint_card.value)
                }
                1 => {
                    Hinted::Color(hint_card.color)
                }
                2 => {
                    // Any value hint for a card other than the first
                    let mut hint_option_set = HashSet::new();
                    for card in hand {
                        if card.value != hint_card.value {
                            hint_option_set.insert(Hinted::Value(card.value));
                        }
                    }
                    self.get_best_hint_of_options(hint_player, hint_option_set)
                }
                3 => {
                    // Any color hint for a card other than the first
                    let mut hint_option_set = HashSet::new();
                    for card in hand {
                        if card.color != hint_card.color {
                            hint_option_set.insert(Hinted::Color(card.color));
                        }
                    }
                    self.get_best_hint_of_options(hint_player, hint_option_set)
                }
                _ => {
                    panic!("Invalid hint type")
                }
            }
        };

        TurnChoice::Hint(Hint {
            player: hint_player,
            hinted: hinted,
        })
    }

    fn infer_from_hint(&mut self, hint: &Hint, result: &Vec<bool>) {
        let hinter = self.last_view.board.player;

        let info_per_player: Vec<Player> = self.get_other_players_starting_after(hinter).iter().map(
            |player| { self.get_info_per_player(*player)  }
        ).collect();
        let total_info = info_per_player.iter().fold(0, |a, b| a + b);

        let n = self.last_view.board.num_players;

        let player_amt = (n + hint.player - hinter - 1) % n;

        let amt_from_prev_players = info_per_player.iter().take(player_amt as usize).fold(0, |a, b| a + b);
        let hint_info_we_can_give_to_this_player = info_per_player[player_amt as usize];

        let card_index = self.get_index_for_hint(self.get_player_public_info(&hint.player), &self.last_view);
        let hint_type =
            if hint_info_we_can_give_to_this_player == 3 {
                if result[card_index] {
                    match hint.hinted {
                        Hinted::Value(_) => 0,
                        Hinted::Color(_) => 1,
                    }
                } else {
                    2
                }
            } else {
                if result[card_index] {
                    match hint.hinted {
                        Hinted::Value(_) => 0,
                        Hinted::Color(_) => 1,
                    }
                } else {
                    match hint.hinted {
                        Hinted::Value(_) => 2,
                        Hinted::Color(_) => 3,
                    }
                }
            };

        let hint_value = amt_from_prev_players + hint_type;

        let mod_info = ModulusInformation::new(total_info, hint_value);

        self.update_from_hint_sum(mod_info);
    }
}

impl PlayerStrategy for InformationPlayerStrategy {
    fn decide(&mut self, _: &BorrowedGameView) -> TurnChoice {
        // we already stored the view
        let view = &self.last_view;

        for player in view.board.get_players() {
           let hand_info = self.get_player_public_info(&player);
            debug!("Current state of hand_info for {}:", player);
            for (i, card_table) in hand_info.iter().enumerate() {
                debug!("  Card {}: {}", i, card_table);
            }
        }

        let private_info = self.get_private_info(view);
        // debug!("My info:");
        // for (i, card_table) in private_info.iter().enumerate() {
        //     debug!("{}: {}", i, card_table);
        // }

        let playable_cards = private_info.iter().enumerate().filter(|&(_, card_table)| {
            card_table.probability_is_playable(&view.board) == 1.0
        }).collect::<Vec<_>>();

        if playable_cards.len() > 0 {
            // play the best playable card
            // the higher the play_score, the better to play
            let mut play_score = -1.0;
            let mut play_index = 0;

            for (index, card_table) in playable_cards {
                let score = self.get_average_play_score(view, card_table);
                if score > play_score {
                    play_score = score;
                    play_index = index;
                }
            }

            return TurnChoice::Play(play_index)
        }

        let discard_threshold =
            view.board.total_cards
            - (COLORS.len() * VALUES.len()) as u32
            - (view.board.num_players * view.board.hand_size);
        let soft_discard_threshold = if view.board.num_players < 5 {
            discard_threshold - 5
        } else {
            discard_threshold
        }; // TODO something more principled.

        // make a possibly risky play
        // TODO: consider removing this, if we improve information transfer
        if view.board.lives_remaining > 1 &&
           view.board.discard_size() <= discard_threshold
        {
            let mut risky_playable_cards = private_info.iter().enumerate().filter(|&(_, card_table)| {
                // card is either playable or dead
                card_table.probability_of_predicate(&|card| {
                    view.board.is_playable(card) || view.board.is_dead(card)
                }) == 1.0
            }).map(|(i, card_table)| {
                let p = card_table.probability_is_playable(&view.board);
                (i, card_table, p)
            }).collect::<Vec<_>>();

            if risky_playable_cards.len() > 0 {
                risky_playable_cards.sort_by(|c1, c2| {
                    c2.2.partial_cmp(&c1.2).unwrap_or(Ordering::Equal)
                });

                let maybe_play = risky_playable_cards[0];
                if maybe_play.2 > 0.75 {
                    return TurnChoice::Play(maybe_play.0);
                }
            }
        }

        let public_useless_indices = self.find_useless_cards(view, &self.get_my_public_info());
        let useless_indices = self.find_useless_cards(view, &private_info);

        if view.board.discard_size() <= soft_discard_threshold {
            // if anything is totally useless, discard it
            if public_useless_indices.len() > 1 {
                let info = self.get_hint_sum_info(public_useless_indices.len() as u32, view);
                return TurnChoice::Discard(public_useless_indices[info.value as usize]);
            } else if useless_indices.len() > 0 {
                // TODO: have opponents infer that i knew a card was useless
                // TODO: after that, potentially prefer useless indices that arent public
                return TurnChoice::Discard(useless_indices[0]);
            }
        }

        // hinting is better than discarding dead cards
        // (probably because it stalls the deck-drawing).
        if view.board.hints_remaining > 0 {
            if view.someone_else_can_play() {
                return self.get_hint();
            }
        }

        // if anything is totally useless, discard it
        if public_useless_indices.len() > 1 {
            let info = self.get_hint_sum_info(public_useless_indices.len() as u32, view);
            return TurnChoice::Discard(public_useless_indices[info.value as usize]);
        } else if useless_indices.len() > 0 {
            return TurnChoice::Discard(useless_indices[0]);
        }

        // NOTE: the only conditions under which we would discard a potentially useful card:
        // - we have no known useless cards
        // - there are no hints remaining OR nobody else can play

        // Play the best discardable card
        let mut compval = 0.0;
        let mut index = 0;
        for (i, card_table) in private_info.iter().enumerate() {
            let probability_is_seen = card_table.probability_of_predicate(&|card| {
                view.can_see(card)
            });
            let my_compval =
                20.0 * probability_is_seen
                + 10.0 * card_table.probability_is_dispensable(&view.board)
                + card_table.average_value();

            if my_compval > compval {
                compval = my_compval;
                index = i;
            }
        }
        TurnChoice::Discard(index)
    }

    fn update(&mut self, turn_record: &TurnRecord, view: &BorrowedGameView) {
        match turn_record.choice {
            TurnChoice::Hint(ref hint) =>  {
                if let &TurnResult::Hint(ref matches) = &turn_record.result {
                    self.infer_from_hint(hint, matches);
                    self.update_public_info_for_hint(hint, matches);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
            TurnChoice::Discard(index) => {
                let known_useless_indices = self.find_useless_cards(
                    &self.last_view, &self.get_player_public_info(&turn_record.player)
                );

                if known_useless_indices.len() > 1 {
                    // unwrap is safe because *if* a discard happened, and there were known
                    // dead cards, it must be a dead card
                    let value = known_useless_indices.iter().position(|&i| i == index).unwrap();
                    self.update_from_hint_sum(ModulusInformation::new(
                        known_useless_indices.len() as u32, value as u32
                    ));
                }

                if let &TurnResult::Discard(ref card) = &turn_record.result {
                    self.update_public_info_for_discard_or_play(view, &turn_record.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
            TurnChoice::Play(index) =>  {
                if let &TurnResult::Play(ref card, _) = &turn_record.result {
                    self.update_public_info_for_discard_or_play(view, &turn_record.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
        }
        self.last_view = OwnedGameView::clone_from(view);
    }
}
