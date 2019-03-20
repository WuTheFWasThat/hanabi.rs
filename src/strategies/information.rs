use fnv::{FnvHashMap, FnvHashSet};
use std::cmp::Ordering;
use float_ord::*;

use strategy::*;
use game::*;
use helpers::*;
use strategies::hat_helpers::*;

// TODO: use random extra information - i.e. when casting up and down,
// we sometimes have 2 choices of value to choose

type PropertyPredicate = fn(&BoardState, &Card) -> bool;

struct CardHasProperty
{
    index: usize,
    property: PropertyPredicate,
}
impl Question for CardHasProperty
{
    fn info_amount(&self) -> u32 { 2 }
    fn answer(&self, hand: &Cards, board: &BoardState) -> u32 {
        let ref card = hand[self.index];
        if (self.property)(board, card) { 1 } else { 0 }
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        board: &BoardState,
    ) {
        let ref mut card_table = hand_info[self.index];
        let possible = card_table.get_possibilities();
        for card in &possible {
            if (self.property)(board, card) {
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

/// For some list of questions l, the question `AdditiveComboQuestion { questions : l }` asks:
/// "What is the first question in the list `l` that has a nonzero answer, and what is its
/// answer?"
/// If all questions in `l` have the answer `0`, this question has the answer `0` as well.
///
/// It's named that way because the `info_amount` grows additively with the `info_amount`s of
/// the questions in `l`.
struct AdditiveComboQuestion {
    questions: Vec<Box<Question>>,
}
impl Question for AdditiveComboQuestion {
    fn info_amount(&self) -> u32 {
        self.questions.iter().map(|q| { q.info_amount() - 1 }).sum::<u32>() + 1
    }
    fn answer(&self, hand: &Cards, board: &BoardState) -> u32 {
        let mut toadd = 1;
        for q in &self.questions {
            let q_answer = q.answer(hand, board);
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
        board: &BoardState,
    ) {
        if answer == 0 {
            answer = self.info_amount();
        }
        answer -= 1;
        for q in &self.questions {
            if answer < q.info_amount() - 1 {
                q.acknowledge_answer(answer+1, hand_info, board);
                return;
            } else {
                q.acknowledge_answer(0, hand_info, board);
                answer -= q.info_amount() - 1;
            }
        }
        assert!(answer == 0);
    }
}

#[derive(Debug)]
struct CardPossibilityPartition {
    index: usize,
    n_partitions: u32,
    partition: FnvHashMap<Card, u32>,
}
impl CardPossibilityPartition {
    fn new(
        index: usize, max_n_partitions: u32, card_table: &CardPossibilityTable, board: &BoardState
    ) -> CardPossibilityPartition {
        let mut cur_block = 0;
        let mut partition = FnvHashMap::default();
        let mut n_partitions = 0;

        let has_dead = card_table.probability_is_dead(&board) != 0.0;

        // TODO: group things of different colors and values?
        let mut effective_max = max_n_partitions;
        if has_dead {
            effective_max -= 1;
        };

        for card in card_table.get_possibilities() {
            if !board.is_dead(&card) {
                partition.insert(card.clone(), cur_block);
                cur_block = (cur_block + 1) % effective_max;
                if n_partitions < effective_max {
                    n_partitions += 1;
                }
            }
        }

        if has_dead {
            for card in card_table.get_possibilities() {
                if board.is_dead(&card) {
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
    fn answer(&self, hand: &Cards, _: &BoardState) -> u32 {
        let ref card = hand[self.index];
        *self.partition.get(&card).unwrap()
    }
    fn acknowledge_answer(
        &self,
        answer: u32,
        hand_info: &mut HandInfo<CardPossibilityTable>,
        _: &BoardState,
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

#[derive(Eq,PartialEq,Clone)]
struct MyPublicInformation {
    // For each player, store the HandInfo, but also store, for each index, whether we already
    // updated on that card being determined.
    player_info: FnvHashMap<Player, (HandInfo<CardPossibilityTable>, Vec<bool>)>,
    card_counts: CardCounts, // what any newly drawn card should be
    board: BoardState, // TODO: maybe we should store an appropriately lifetimed reference?
}

impl MyPublicInformation {
    fn borrow_player_info_mut(&mut self, player: &Player) -> &mut HandInfo<CardPossibilityTable> {
        &mut self.player_info.get_mut(player).unwrap().0
    }

    fn borrow_player_info(&self, player: &Player) -> &HandInfo<CardPossibilityTable> {
        &self.player_info.get(player).unwrap().0
    }

    fn get_other_players_starting_after(&self, player: Player) -> Vec<Player> {
        let n = self.board.num_players;
        (0 .. n - 1).into_iter().map(|i| { (player + 1 + i) % n }).collect()
    }

    fn get_private_info(&self, view: &OwnedGameView) -> HandInfo<CardPossibilityTable> {
        let mut info = self.get_player_info(&view.player);
        for card_table in info.iter_mut() {
            for (other_player, hand) in &view.other_hands {
                let (_, was_determined) = self.player_info.get(other_player).unwrap();
                for (card_was_determined, card) in was_determined.iter().zip(hand.iter()) {
                    if !card_was_determined {
                        card_table.decrement_weight_if_possible(card);
                    }
                }
            }
        }
        info
    }


    // Returns the number of ways to hint the player.
    fn get_info_per_player(&self, player: Player) -> u32 {
        // Determine if both:
        //  - it is public that there are at least two colors
        //  - it is public that there are at least two numbers

        let info = self.borrow_player_info(&player);

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
    }

    fn get_hint_index_score(&self, card_table: &CardPossibilityTable) -> i32 {
        if card_table.probability_is_dead(&self.board) == 1.0 {
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

    fn get_index_for_hint(&self, player: &Player) -> usize {
        let mut scores = self.borrow_player_info(player).iter().enumerate().map(|(i, card_table)| {
            let score = self.get_hint_index_score(card_table);
            (-score, i)
        }).collect::<Vec<_>>();
        scores.sort();
        scores[0].1
    }

    // TODO: refactor out the common parts of get_hint and update_from_hint_choice
    fn get_hint(&mut self, view: &OwnedGameView) -> Vec<Hint> {
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

        let hinter = view.player;
        let info_per_player: Vec<_> = self.get_other_players_starting_after(hinter).into_iter().map(
            |player| { self.get_info_per_player(player) }
        ).collect();
        let total_info = info_per_player.iter().sum();
        // FIXME explain and clean up
        let card_indices: Vec<_> = self.get_other_players_starting_after(hinter).into_iter().map(
            |player| { self.get_index_for_hint(&player) }
        ).collect();

        let hint_info = self.get_hat_sum(total_info, view);

        //let hint_type = hint_info.value % 3;
        //let player_amt = (hint_info.value - hint_type) / 3;
        let mut hint_type = hint_info.value;
        let mut player_amt = 0;
        while hint_type >= info_per_player[player_amt] {
            hint_type -= info_per_player[player_amt];
            player_amt += 1;
        }
        let hint_info_we_can_give_to_this_player = info_per_player[player_amt];

        let hint_player = (hinter + 1 + (player_amt as u32)) % view.board.num_players;

        let hand = view.get_hand(&hint_player);
        let card_index = card_indices[player_amt];
        let hint_card = &hand[card_index];

        let hint_option_set = if hint_info_we_can_give_to_this_player == 3 {
            match hint_type {
                0 => {
                    vec![Hinted::Value(hint_card.value)]
                }
                1 => {
                    vec![Hinted::Color(hint_card.color)]
                }
                2 => {
                    // NOTE: this doesn't do that much better than just hinting
                    // the first thing that doesn't match the hint_card
                    let mut hint_option_set = Vec::new();
                    for card in hand {
                        if card.color != hint_card.color {
                            hint_option_set.push(Hinted::Color(card.color));
                        }
                        if card.value != hint_card.value {
                            hint_option_set.push(Hinted::Value(card.value));
                        }
                    }
                    hint_option_set
                }
                _ => {
                    panic!("Invalid hint type")
                }
            }
        } else {
            match hint_type {
                0 => {
                    vec![Hinted::Value(hint_card.value)]
                }
                1 => {
                    vec![Hinted::Color(hint_card.color)]
                }
                2 => {
                    // Any value hint for a card other than the first
                    let mut hint_option_set = Vec::new();
                    for card in hand {
                        if card.value != hint_card.value {
                            hint_option_set.push(Hinted::Value(card.value));
                        }
                    }
                    hint_option_set
                }
                3 => {
                    // Any color hint for a card other than the first
                    let mut hint_option_set = Vec::new();
                    for card in hand {
                        if card.color != hint_card.color {
                            hint_option_set.push(Hinted::Color(card.color));
                        }
                    }
                    hint_option_set
                }
                _ => {
                    panic!("Invalid hint type")
                }
            }
        };
        hint_option_set.into_iter().collect::<FnvHashSet<_>>().into_iter().map(|hinted| {
            Hint {
                player: hint_player,
                hinted: hinted,
            }
        }).collect()
    }

    fn decode_hint_choice(&self, hint: &Hint, result: &Vec<bool>) -> ModulusInformation {
        let hinter = self.board.player;

        let info_per_player: Vec<_> = self.get_other_players_starting_after(hinter).into_iter().map(
            |player| { self.get_info_per_player(player)  }
        ).collect();
        let total_info = info_per_player.iter().sum();

        let n = self.board.num_players;

        let player_amt = (n + hint.player - hinter - 1) % n;

        let amt_from_prev_players = info_per_player.iter().take(player_amt as usize).fold(0, |a, b| a + b);
        let hint_info_we_can_give_to_this_player = info_per_player[player_amt as usize];

        let card_index = self.get_index_for_hint(&hint.player);
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

        ModulusInformation::new(total_info, hint_value)
    }

    fn update_from_hint_choice(&mut self, hint: &Hint, matches: &Vec<bool>, view: &OwnedGameView) {
        let info = self.decode_hint_choice(hint, matches);
        self.update_from_hat_sum(info, view);
    }

    fn update_from_hint_matches(&mut self, hint: &Hint, matches: &Vec<bool>) {
        {
            let info = self.borrow_player_info_mut(&hint.player);
            info.update_for_hint(&hint.hinted, matches);
        }
        self.update_other_info();
    }

    fn knows_playable_card(&self, player: &Player) -> bool {
        self.borrow_player_info(player).iter().any(|table| {
            table.probability_is_playable(&self.board) == 1.0
        })
    }

    fn someone_else_needs_hint(&self, view: &OwnedGameView) -> bool {
        // Does another player have a playable card, but doesn't know it?
        view.get_other_players().iter().any(|player| {
            let has_playable_card = view.get_hand(&player).iter().any(|card| {
                view.get_board().is_playable(card)
            });
            has_playable_card && !self.knows_playable_card(&player)
        })
    }

    fn update_noone_else_needs_hint(&mut self) {
        // If it becomes public knowledge that someone_else_needs_hint() returns false,
        // update accordingly.
        for player in self.board.get_players() {
            if player != self.board.player && !self.knows_playable_card(&player) {
                // If player doesn't know any playable cards, player doesn't have any playable
                // cards.
                let mut hand_info = self.get_player_info(&player);
                for ref mut card_table in hand_info.iter_mut() {
                    let possible = card_table.get_possibilities();
                    for card in &possible {
                        if self.board.is_playable(card) {
                            card_table.mark_false(card);
                        }
                    }
                }
                self.set_player_info(&player, hand_info);
            }
        }
        self.update_other_info();
    }

    fn update_from_discard_or_play_result(
        &mut self,
        new_view: &BorrowedGameView,
        player: &Player,
        index: usize,
        card: &Card
    ) {
        let new_card_table = CardPossibilityTable::from(&self.card_counts);
        let was_known_determined = {
            let (ref mut hand_info, ref mut known_determined) = self.player_info.get_mut(player).unwrap();
            assert!(hand_info[index].is_possible(card));
            hand_info.remove(index);
            let was_known_determined = known_determined.remove(index);

            // push *before* incrementing public counts
            if hand_info.len() < new_view.hand_size(&player) {
                hand_info.push(new_card_table);
                known_determined.push(false);
            }
            was_known_determined
        };
        if !was_known_determined {
            self.decrement_weights_for_card(card);
            self.update_other_info();
        }
    }

    /// Once we know a specific card is somewhere (a hand location or the discard pile), decrement
    /// its weights everywhere else.
    fn decrement_weights_for_card(&mut self, card: &Card) {
        for player in self.board.get_players() {
            let info = self.borrow_player_info_mut(&player);
            for card_table in info.iter_mut() {
                if !card_table.is_determined() {
                    card_table.decrement_weight_if_possible(card);
                }
            }
        }
        self.card_counts.increment(card);
    }
}

impl PublicInformation for MyPublicInformation {
    fn new(board: &BoardState) -> Self {
        let player_info = board.get_players().map(|player| {
            let hand_info = HandInfo::new(board.hand_size);
            let is_determined = vec![false; board.hand_size as usize];
            (player, (hand_info, is_determined))
        }).collect::<FnvHashMap<_,_>>();
        MyPublicInformation {
            player_info: player_info,
            card_counts: CardCounts::new(),
            board: board.clone(),
        }
    }

    fn set_board(&mut self, board: &BoardState) {
        self.board = board.clone();
    }

    fn get_player_info(&self, player: &Player) -> HandInfo<CardPossibilityTable> {
        self.borrow_player_info(player).clone()
    }

    fn set_player_info(&mut self, player: &Player, hand_info: HandInfo<CardPossibilityTable>) {
        *self.borrow_player_info_mut(player) = hand_info;
    }

    fn agrees_with(&self, other: Self) -> bool {
        *self == other
    }

    fn update_other_info(&mut self) {
        loop {
            let mut updated_cards = Vec::new();
            for player in self.board.get_players() {
                let (hand_info, known_determined) = self.player_info.get_mut(&player).unwrap();
                let zipped_iter = known_determined.iter_mut().zip(hand_info.iter());
                for (is_known_determined, card_table) in zipped_iter {
                    if !*is_known_determined && card_table.is_determined() {
                        updated_cards.push(card_table.get_card().unwrap());
                        *is_known_determined = true;
                    }
                }
            }
            if updated_cards.is_empty() {
                break;
            }
            for card in updated_cards.into_iter() {
                self.decrement_weights_for_card(&card);
            }
        }
    }

    fn ask_question(
        &self,
        _me: &Player,
        hand_info: &HandInfo<CardPossibilityTable>,
        total_info: u32,
    ) -> Option<Box<Question>> {
        // Changing anything inside this function will not break the information transfer
        // mechanisms!

        let augmented_hand_info_raw = hand_info.iter().cloned().enumerate().filter_map(|(i, card_table)| {
            let p_play = card_table.probability_is_playable(&self.board);
            let p_dead = card_table.probability_is_dead(&self.board);
            Some((i, p_play, p_dead))
        }).collect::<Vec<_>>();
        let know_playable_card = augmented_hand_info_raw.iter().any(|&(_, p_play, _)| p_play == 1.0);
        let know_dead_card     = augmented_hand_info_raw.iter().any(|&(_, _, p_dead)| p_dead == 1.0);

        // We don't need to find out anything about cards that are determined or dead.
        let augmented_hand_info = augmented_hand_info_raw.into_iter().filter(|&(i, _, p_dead)| {
            if p_dead == 1.0 { false }
            else if hand_info[i].is_determined() { false }
            else { true }
        }).collect::<Vec<_>>();

        if !know_playable_card {
            // Vector of tuples (ask_dead, i, p_yes), where ask_dead=false means we'll
            // ask if the card at i is playable, and ask_dead=true means we ask if the card at i is
            // dead. p_yes is the probability the answer is nonzero.
            let mut to_ask: Vec<(bool, usize, f32)> = augmented_hand_info.iter().filter_map(|&(i, p_play, _)| {
                if p_play == 0.0 { None }
                else { Some((false, i, p_play)) }
            }).collect();
            if !know_dead_card {
                to_ask.extend(augmented_hand_info.iter().filter_map(|&(i, _, p_dead)| {
                    if p_dead == 0.0 { None }
                    else { Some((true, i, p_dead)) }
                }));
            }

            let combo_question_capacity = (total_info - 1) as usize;
            if to_ask.len() > combo_question_capacity {
                // The questions don't fit into an AdditiveComboQuestion.
                // Sort by type (ask_dead=false first), then by p_yes (bigger first)
                to_ask.sort_by_key(|&(ask_dead, _, p_yes)| {(ask_dead, FloatOrd(-p_yes))});
                to_ask.truncate(combo_question_capacity);
            }

            // Sort by type (ask_dead=false first), then by p_yes (smaller first), since it's
            // better to put lower-probability-of-playability/death cards first: The difference
            // only matters if we find a playable/dead card, and conditional on that, it's better
            // to find out about as many non-playable/non-dead cards as possible.
            to_ask.sort_by_key(|&(ask_dead, _, p_yes)| {(ask_dead, FloatOrd(p_yes))});
            let questions = to_ask.into_iter().map(|(ask_dead, i, _)| -> Box<Question> {
                if ask_dead { Box::new(q_is_dead(i)) }
                else        { Box::new(q_is_playable(i)) }
            }).collect::<Vec<_>>();
            if questions.len() > 0 {
                return Some(Box::new(AdditiveComboQuestion { questions }))
            }
        }

        let ask_play_score = |p_play: f32| FloatOrd((p_play-0.7).abs());
        let mut ask_play = augmented_hand_info.iter().filter(|&&(_, p_play, _)| {
            ask_play_score(p_play) < FloatOrd(0.2)
        }).cloned().collect::<Vec<_>>();
        ask_play.sort_by_key(|&(i, p_play, _)| (ask_play_score(p_play), i));
        if let Some(&(i, _, _)) = ask_play.get(0) {
            return Some(Box::new(q_is_playable(i)));
        }

        let mut ask_partition = augmented_hand_info;
        // sort by probability of death (lowest first), then by index
        ask_partition.sort_by_key(|&(i, _, p_death)| {
            (FloatOrd(p_death), i)
        });
        if let Some(&(i, _, _)) = ask_partition.get(0) {
            let question = CardPossibilityPartition::new(i, total_info, &hand_info[i], &self.board);
            Some(Box::new(question))
        } else {
            None
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
        Box::new(InformationPlayerStrategy {
            me: player,
            public_info: MyPublicInformation::new(view.board),
            new_public_info: None,
            last_view: OwnedGameView::clone_from(view),
        })
    }
}

pub struct InformationPlayerStrategy {
    me: Player,
    public_info: MyPublicInformation,
    // Inside decide(), modify a copy of public_info and put it here. After that, when
    // calling update, check that the updated public_info matches new_public_info.
    new_public_info: Option<MyPublicInformation>,
    last_view: OwnedGameView, // the view on the previous turn
}

impl InformationPlayerStrategy {
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

    fn find_useless_cards(&self, board: &BoardState, hand: &HandInfo<CardPossibilityTable>) -> Vec<usize> {
        let mut useless: FnvHashSet<usize> = FnvHashSet::default();
        let mut seen: FnvHashMap<Card, usize> = FnvHashMap::default();

        for (i, card_table) in hand.iter().enumerate() {
            if card_table.probability_is_dead(board) == 1.0 {
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

    // how good is it to give this hint to this player?
    fn hint_goodness(&self, hint: &Hint, view: &OwnedGameView) -> f32 {
        // This gets called after self.public_info.get_hint(), which modifies the public
        // info to include information gained through question answering. Therefore, we only
        // simulate information gained through the hint result here.

        let hint_player = &hint.player;
        let hinted = &hint.hinted;
        let hand = view.get_hand(&hint_player);
        let mut hand_info = self.public_info.get_player_info(&hint_player);

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

    fn get_best_hint_of_options(&self, mut hints: Vec<Hint>) -> Hint {
        if hints.len() == 1 {
            return hints.remove(0);
        }
        let view = &self.last_view;

        // using hint goodness barely helps
        let mut hint_options = hints.into_iter().map(|hint| {
            (self.hint_goodness(&hint, view), hint)
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

    /// Decide on a move. At the same time, simulate the impact of that move on the public
    /// information state by modifying `public_info`. Since `self` is immutable and since our
    /// public information state change will be compared against the change in the corresponding
    /// call to `update_wrapped`, nothing we do here will let our public information state silently
    /// get out of sync with other players' public information state!
    fn decide_wrapped(&mut self, public_info: &mut MyPublicInformation) -> TurnChoice {
        // we already stored the view
        let view = &self.last_view;
        let me = &view.player;

        for player in view.board.get_players() {
           let hand_info = public_info.get_player_info(&player);
            debug!("Current state of hand_info for {}:", player);
            for (i, card_table) in hand_info.iter().enumerate() {
                debug!("  Card {}: {}", i, card_table);
            }
        }

        let private_info = public_info.get_private_info(view);
        // debug!("My info:");
        // for (i, card_table) in private_info.iter().enumerate() {
        //     debug!("{}: {}", i, card_table);
        // }

        // If possible, play the best playable card
        // the higher the play_score, the better to play
        let mut playable_cards = private_info.iter().enumerate().filter_map(|(i, card_table)| {
            if card_table.probability_is_playable(&view.board) != 1.0 { return None; }
            Some((i, self.get_average_play_score(view, card_table)))
        }).collect::<Vec<_>>();
        playable_cards.sort_by_key(|&(i, play_score)| (FloatOrd(-play_score), i));
        if let Some(&(play_index, _)) = playable_cards.get(0) {
            return TurnChoice::Play(play_index)
        }

        // If the deck ran out, we know we have a playable card somewhere, and there's only one place
        // where it could be, play it.
        // NOTE: This is very close to unnecessary since we're aggressively making risky plays at
        // the end of the game.
        // TODO: Replace this "ad hoc deduction" with something better integrated into our strategy
        if view.board.deck_size == 0 {
            let possibly_playable_cards = private_info.iter().enumerate().filter(|&(_, card_table)| {
                card_table.probability_is_playable(&view.board) > 0.0
            }).collect::<Vec<_>>();
            if possibly_playable_cards.len() == 1 {
                let (index, _) = possibly_playable_cards[0];
                return TurnChoice::Play(index);
            }
        }

        let discard_threshold =
            view.board.total_cards
            - (COLORS.len() * VALUES.len()) as u32
            - (view.board.num_players * view.board.hand_size);

        // Our willingness to make risky plays. If None, we won't, if Some(p) we play a card if it
        // succeeds with probability > p.
        let risky_play_threshold =
            if view.board.lives_remaining <= 1 { None }
            // NOTE: Once the deck runs out, our calculated "success probabilities" are pretty far off
            // from the probabilities a smarter reasoner would come up with. Once we improve how we
            // assign probabilities to cards, we should check if a threshold other than 0.0 is better.
            else if view.board.deck_size == 0 { Some(0.0) }
            else if view.board.discard_size() <= discard_threshold { Some(0.75) }
            else { None };
        if let Some(risky_play_threshold) = risky_play_threshold {
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
                if maybe_play.2 > risky_play_threshold {
                    return TurnChoice::Play(maybe_play.0);
                }
            }
        }

        let useless_indices = self.find_useless_cards(&view.board, &private_info);

        // NOTE When changing this, make sure to keep the "discard" branch of update() up to date!
        let will_hint =
            if view.board.hints_remaining > 0 && public_info.someone_else_needs_hint(view) { true }
            else if view.board.discard_size() <= discard_threshold && useless_indices.len() > 0 { false }
            // hinting is better than discarding dead cards
            // (probably because it stalls the deck-drawing).
            else if view.board.hints_remaining > 0 && view.someone_else_can_play() { true }
            else if view.board.hints_remaining > 4 { true }
            // this is the only case in which we discard a potentially useful card.
            else { false };

        if will_hint {
            let hint_set = public_info.get_hint(view);
            let hint = self.get_best_hint_of_options(hint_set);
            return TurnChoice::Hint(hint);
        }

        if self.last_view.board.hints_remaining > 0 {
            public_info.update_noone_else_needs_hint();
        }

        // This block has to be after the update_noone_else_needs_hint() since this call could
        // have expanded the set of public useless indices.
        let public_useless_indices = self.find_useless_cards(&view.board, &public_info.get_player_info(me));

        // if anything is totally useless, discard it
        if public_useless_indices.len() > 1 {
            let info = public_info.get_hat_sum(public_useless_indices.len() as u32, view);
            return TurnChoice::Discard(public_useless_indices[info.value as usize]);
        } else if useless_indices.len() > 0 {
            // TODO: have opponents infer that i knew a card was useless
            // TODO: after that, potentially prefer useless indices that arent public
            return TurnChoice::Discard(useless_indices[0]);
        }

        // Make the least risky discard.
        let mut cards_by_discard_value = private_info.iter().enumerate().map(|(i, card_table)| {
            let probability_is_seen = card_table.probability_of_predicate(&|card| {
                view.can_see(card)
            });
            let compval =
                20.0 * probability_is_seen
                + 10.0 * card_table.probability_is_dispensable(&view.board)
                + card_table.average_value();
            (i, compval)
        }).collect::<Vec<_>>();
        cards_by_discard_value.sort_by_key(|&(i, compval)| (FloatOrd(-compval), i));
        let (index, _) = cards_by_discard_value[0];
        TurnChoice::Discard(index)
    }

    /// Update the public information. The "update" operations on the public information state have to
    /// exactly match the corresponding "choice" operations in `decide_wrapped()`.
    ///
    /// We don't have to update on the "turn result" here. If the turn was a hint, we get the
    /// matches in order to understand the "intention" behind the hint, but we do not need to
    /// update on what the hint says about the hinted player's cards directly. (This is done in the
    /// call to `update_hint_matches()` inside `update()`.
    fn update_wrapped(
        &mut self,
        turn_player: &Player,
        turn_choice: &TurnChoice,
        hint_matches: Option<&Vec<bool>>,
    ) {
        match turn_choice {
            TurnChoice::Hint(ref hint) =>  {
                let matches = hint_matches.unwrap();
                self.public_info.update_from_hint_choice(hint, matches, &self.last_view);
            }
            TurnChoice::Discard(index) => {
                if self.last_view.board.hints_remaining > 0 {
                    self.public_info.update_noone_else_needs_hint();
                }

                let known_useless_indices = self.find_useless_cards(
                    &self.last_view.board, &self.public_info.get_player_info(turn_player)
                );
                if known_useless_indices.len() > 1 {
                    // unwrap is safe because *if* a discard happened, and there were known
                    // dead cards, it must be a dead card
                    let value = known_useless_indices.iter().position(|&i| i == *index).unwrap();
                    let info = ModulusInformation::new(known_useless_indices.len() as u32, value as u32);
                    self.public_info.update_from_hat_sum(info, &self.last_view);
                }
            }
            TurnChoice::Play(_index) => {
                // TODO: Maybe we can transfer information through plays as well?
            }
        }
    }
}

impl PlayerStrategy for InformationPlayerStrategy {
    fn decide(&mut self, _: &BorrowedGameView) -> TurnChoice {
        let mut public_info = self.public_info.clone();
        let turn_choice = self.decide_wrapped(&mut public_info);
        self.new_public_info = Some(public_info);
        turn_choice
    }

    fn update(&mut self, turn_record: &TurnRecord, view: &BorrowedGameView) {
        let hint_matches = if let &TurnResult::Hint(ref matches) = &turn_record.result {
            Some(matches)
        } else { None };
        self.update_wrapped(&turn_record.player, &turn_record.choice, hint_matches);
        if let Some(new_public_info) = self.new_public_info.take() {
            if !self.public_info.agrees_with(new_public_info) {
                panic!("The change made to public_info in self.decide_wrapped differs from \
                        the corresponding change in self.update_wrapped!");
            }
        }
        match turn_record.choice {
            TurnChoice::Hint(ref hint) =>  {
                if let &TurnResult::Hint(ref matches) = &turn_record.result {
                    self.public_info.update_from_hint_matches(hint, matches);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
            TurnChoice::Discard(index) => {
                if let &TurnResult::Discard(ref card) = &turn_record.result {
                    self.public_info.update_from_discard_or_play_result(view, &turn_record.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
            TurnChoice::Play(index) =>  {
                if let &TurnResult::Play(ref card, _) = &turn_record.result {
                    self.public_info.update_from_discard_or_play_result(view, &turn_record.player, index, card);
                } else {
                    panic!("Got turn choice {:?}, but turn result {:?}",
                           turn_record.choice, turn_record.result);
                }
            }
        }
        self.last_view = OwnedGameView::clone_from(view);
        self.public_info.set_board(view.board);
    }
}
