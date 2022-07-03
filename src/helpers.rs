use std::cmp::Eq;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::{Index,IndexMut};
use std::hash::Hash;
use std::convert::From;
use std::slice;

use game::*;

// trait representing information about a card
pub trait CardInfo {
    fn new() -> Self;

    // whether the card is possible
    fn is_possible(&self, card: &Card) -> bool;

    // mark all current possibilities for the card
    // this should generally be overridden, for efficiency
    fn get_possibilities(&self) -> Vec<Card> {
        let mut v = Vec::new();
        for &color in COLORS.iter() {
            for &value in VALUES.iter() {
                let card = Card::new(color, value);
                if self.is_possible(&card) {
                    v.push(card);
                }
            }
        }
        v
    }

    // get probability weight for the card
    #[allow(unused_variables)]
    fn get_weight(&self, card: &Card) -> f32 {
        1.
    }

    fn get_weighted_possibilities(&self) -> Vec<(Card, f32)> {
        self.get_possibilities().into_iter()
            .map(|card| {
                let weight = self.get_weight(&card);
                (card, weight)
            }).collect::<Vec<_>>()
    }

    fn total_weight(&self) -> f32 {
        self.get_possibilities().iter()
            .map(|card| self.get_weight(card))
            .fold(0.0, |a, b| a+b)
    }

    fn weighted_score<T>(&self, score_fn: &dyn Fn(&Card) -> T) -> f32
        where f32: From<T>
    {
        let mut total_score = 0.;
        let mut total_weight = 0.;
        for card in self.get_possibilities() {
            let weight = self.get_weight(&card);
            let score = f32::from(score_fn(&card));
            total_weight += weight;
            total_score += weight * score;
        }
        total_score / total_weight
    }

    fn average_value(&self) -> f32 {
        self.weighted_score(&|card| card.value as f32 )
    }

    fn probability_of_predicate(&self, predicate: &dyn Fn(&Card) -> bool) -> f32 {
        let f = |card: &Card| {
            if predicate(card) { 1.0 } else { 0.0 }
        };
        self.weighted_score(&f)
    }

    fn probability_is_playable(&self, board: &BoardState) -> f32 {
        self.probability_of_predicate(&|card| board.is_playable(card))
    }

    fn probability_is_dead(&self, board: &BoardState) -> f32 {
        self.probability_of_predicate(&|card| board.is_dead(card))
    }

    fn probability_is_dispensable(&self, board: &BoardState) -> f32 {
        self.probability_of_predicate(&|card| board.is_dispensable(card))
    }

    // mark a whole color as false
    fn mark_color_false(&mut self, color: Color);
    // mark a color as correct
    fn mark_color_true(&mut self, color: Color) {
        for &other_color in COLORS.iter() {
            if other_color != color {
                self.mark_color_false(other_color);
            }
        }
    }
    fn mark_color(&mut self, color: Color, is_color: bool) {
        if is_color {
            self.mark_color_true(color);
        } else {
            self.mark_color_false(color);
        }
    }

    // mark a whole value as false
    fn mark_value_false(&mut self, value: Value);
    // mark a value as correct
    fn mark_value_true(&mut self, value: Value) {
        for &other_value in VALUES.iter() {
            if other_value != value {
                self.mark_value_false(other_value);
            }
        }
    }
    fn mark_value(&mut self, value: Value, is_value: bool) {
        if is_value {
            self.mark_value_true(value);
        } else {
            self.mark_value_false(value);
        }
    }
}


// Represents hinted information about possible values of type T
pub trait Info<T> where T: Hash + Eq + Clone {
    // get all a-priori possibilities
    fn get_all_possibilities() -> Vec<T>;

    // get map from values to whether it's possible
    // true means maybe, false means no
    fn get_possibility_set(&self) -> &HashSet<T>;
    fn get_mut_possibility_set(&mut self) -> &mut HashSet<T>;

    // get what is now possible
    fn get_possibilities(&self) -> Vec<T> {
        self.get_possibility_set().iter().cloned().collect::<Vec<T>>()
    }

    fn is_possible(&self, value: T) -> bool {
        self.get_possibility_set().contains(&value)
    }

    fn initialize() -> HashSet<T> {
        Self::get_all_possibilities().iter()
            .cloned().collect::<HashSet<_>>()
    }

    fn mark_true(&mut self, value: T) {
        let possible = self.get_mut_possibility_set();
        possible.clear();
        possible.insert(value);
    }

    fn mark_false(&mut self, value: T) {
        self.get_mut_possibility_set().remove(&value);
    }

    fn mark(&mut self, value: T, info: bool) {
        if info { self.mark_true(value); } else { self.mark_false(value); }
    }
}

#[derive(Debug,Clone)]
pub struct ColorInfo(HashSet<Color>);
impl ColorInfo {
    pub fn new() -> ColorInfo { ColorInfo(ColorInfo::initialize()) }
}
impl Info<Color> for ColorInfo {
    fn get_all_possibilities() -> Vec<Color> { COLORS.to_vec() }
    fn get_possibility_set(&self) -> &HashSet<Color> { &self.0 }
    fn get_mut_possibility_set(&mut self) -> &mut HashSet<Color> { &mut self.0 }
}

#[derive(Debug,Clone)]
pub struct ValueInfo(HashSet<Value>);
impl ValueInfo {
    pub fn new() -> ValueInfo { ValueInfo(ValueInfo::initialize()) }
}
impl Info<Value> for ValueInfo {
    fn get_all_possibilities() -> Vec<Value> { VALUES.to_vec() }
    fn get_possibility_set(&self) -> &HashSet<Value> { &self.0 }
    fn get_mut_possibility_set(&mut self) -> &mut HashSet<Value> { &mut self.0 }
}

// represents information only of the form:
// this color is/isn't possible, this value is/isn't possible
#[derive(Debug,Clone)]
pub struct SimpleCardInfo {
    pub color_info: ColorInfo,
    pub value_info: ValueInfo,
}
impl CardInfo for SimpleCardInfo {
    fn new() -> SimpleCardInfo {
        SimpleCardInfo {
            color_info: ColorInfo::new(),
            value_info: ValueInfo::new(),
        }
    }

    fn get_possibilities(&self) -> Vec<Card> {
        let mut v = Vec::new();
        for &color in self.color_info.get_possibilities().iter() {
            for &value in self.value_info.get_possibilities().iter() {
                v.push(Card::new(color, value));
            }
        }
        v
    }
    fn is_possible(&self, card: &Card) -> bool {
        self.color_info.is_possible(card.color) &&
        self.value_info.is_possible(card.value)

    }
    fn mark_color_false(&mut self, color: Color) {
        self.color_info.mark_false(color);

    }
    fn mark_value_false(&mut self, value: Value) {
        self.value_info.mark_false(value);
    }
}
impl fmt::Display for SimpleCardInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut string = String::new();
        for &color in &COLORS {
            if self.color_info.is_possible(color) {
                string.push(color);
            }
        }
        // while string.len() < COLORS.len() + 1 {
        string.push(' ');
        //}
        for &value in &VALUES {
            if self.value_info.is_possible(value) {
                string.push_str(&format!("{}", value));
            }
        }
        f.pad(&string)
    }
}

// Can represent information of the form:
// this card is/isn't possible
// also, maintains integer weights for the cards
#[derive(Clone,Debug,Eq,PartialEq)]
pub struct CardPossibilityTable {
    possible: HashMap<Card, u32>,
}
impl CardPossibilityTable {
    // mark a possible card as false
    pub fn mark_false(&mut self, card: &Card) {
        self.possible.remove(card);
    }

    // a bit more efficient
    // pub fn borrow_possibilities<'a>(&'a self) -> Vec<&'a Card> {
    //     self.possible.keys().collect::<Vec<_>>()
    // }

    pub fn decrement_weight_if_possible(&mut self, card: &Card) {
        if self.is_possible(card) {
            self.decrement_weight(card);
        }
    }

    pub fn decrement_weight(&mut self, card: &Card) {
        let remove = {
            let weight =
                self.possible.get_mut(card)
                    .expect(&format!("Decrementing weight for impossible card: {}", card));
            *weight -= 1;
            *weight == 0
        };
        if remove {
            self.possible.remove(card);
        }
    }

    pub fn get_card(&self) -> Option<Card> {
        let possibilities = self.get_possibilities();
        if possibilities.len() == 1 {
            Some(possibilities[0].clone())
        } else {
            None
        }
    }

    pub fn is_determined(&self) -> bool {
        self.get_possibilities().len() == 1
    }

    pub fn color_determined(&self) -> bool {
        self.get_possibilities()
            .iter().map(|card| card.color)
            .collect::<HashSet<_>>()
            .len() == 1
    }

    pub fn value_determined(&self) -> bool {
        self.get_possibilities()
            .iter().map(|card| card.value)
            .collect::<HashSet<_>>()
            .len() == 1
    }

    pub fn can_be_color(&self, color: Color) -> bool {
        self.get_possibilities().into_iter().any(|card| card.color == color)
    }

    pub fn can_be_value(&self, value: Value) -> bool {
        self.get_possibilities().into_iter().any(|card| card.value == value)
    }
}
impl <'a> From<&'a CardCounts> for CardPossibilityTable {
    fn from(counts: &'a CardCounts) -> CardPossibilityTable {
        let mut possible = HashMap::new();
        for &color in COLORS.iter() {
            for &value in VALUES.iter() {
                let card = Card::new(color, value);
                let count = counts.remaining(&card);
                if count > 0 {
                    possible.insert(card, count);
                }
            }
        }
        CardPossibilityTable {
            possible,
        }
    }
}
impl CardInfo for CardPossibilityTable {
    fn new() -> CardPossibilityTable {
        Self::from(&CardCounts::new())
    }

    fn is_possible(&self, card: &Card) -> bool {
        self.possible.contains_key(card)
    }
    fn get_possibilities(&self) -> Vec<Card> {
        let mut cards = self.possible.keys().cloned().collect::<Vec<_>>();
        cards.sort();
        cards
    }
    fn mark_color_false(&mut self, color: Color) {
        for &value in VALUES.iter() {
            self.mark_false(&Card::new(color, value));
        }

    }
    fn mark_value_false(&mut self, value: Value) {
        for &color in COLORS.iter() {
            self.mark_false(&Card::new(color, value));
        }
    }
    fn get_weight(&self, card: &Card) -> f32 {
        *self.possible.get(card).unwrap_or(&0) as f32
    }
}
impl fmt::Display for CardPossibilityTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (card, weight) in &self.possible {
            f.write_str(&format!("{} {}, ", weight, card))?;
        }
        Ok(())
    }
}

#[derive(Clone,Eq,PartialEq)]
pub struct HandInfo<T> where T: CardInfo {
    pub hand_info: Vec<T>
}
impl <T> HandInfo<T> where T: CardInfo {
    pub fn new(hand_size: u32) -> Self {
        let hand_info = (0..hand_size).map(|_| T::new()).collect::<Vec<_>>();
        HandInfo {
            hand_info,
        }
    }

    // update for hint to me
    pub fn update_for_hint(&mut self, hinted: &Hinted, matches: &[bool]) {
        match hinted {
            Hinted::Color(color) => {
                for (card_info, &matched) in self.hand_info.iter_mut().zip(matches.iter()) {
                    card_info.mark_color(*color, matched);
                }
            }
            Hinted::Value(value) => {
                for (card_info, &matched) in self.hand_info.iter_mut().zip(matches.iter()) {
                    card_info.mark_value(*value, matched);
                }
            }
        }
    }

    pub fn remove(&mut self, index: usize) -> T { self.hand_info.remove(index) }
    pub fn push(&mut self, card_info: T)        { self.hand_info.push(card_info) }
    pub fn iter_mut(&mut self) -> slice::IterMut<T> { self.hand_info.iter_mut() }
    pub fn iter(&self) -> slice::Iter<T>        { self.hand_info.iter() }
    pub fn len(&self) -> usize                  { self.hand_info.len() }
}
impl <T> Index<usize> for HandInfo<T> where T: CardInfo {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.hand_info[index]
    }
}
impl <T> IndexMut<usize> for HandInfo<T> where T: CardInfo {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.hand_info[index]
    }
}
