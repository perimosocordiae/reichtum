use crate::data_types::{Card, Noble};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    // Token counts: [white, blue, green, red, black, gold]
    pub tokens: [u8; 6],
    // Purchased cards: [white, blue, green, red, black]
    owned: [Vec<u8>; 5],
    // Reserved cards
    reserved: Vec<Card>,
    // Acquired nobles
    pub nobles: Vec<Noble>,
    // VP history: [(round, vp)]
    // NOTE: This is updated by the game state, not the player itself.
    pub vp_history: Vec<(u16, u8)>,
}
impl Player {
    pub fn default() -> Self {
        Self {
            tokens: [0, 0, 0, 0, 0, 0],
            owned: [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            reserved: Vec::new(),
            nobles: Vec::new(),
            vp_history: vec![(0, 0)],
        }
    }
    pub fn num_tokens(&self) -> u8 {
        self.tokens.iter().sum()
    }
    pub fn vp(&self) -> u8 {
        return self.vp_history.last().unwrap().1;
    }
    pub fn purchasing_power(&self, include_tokens: bool) -> [u8; 5] {
        let mut power: [u8; 5] = [0, 0, 0, 0, 0];
        if include_tokens {
            power.copy_from_slice(&self.tokens[0..5]);
        }
        for (i, cards) in self.owned.iter().enumerate() {
            power[i] += cards.len() as u8;
        }
        power
    }
    pub fn can_buy(&self, card: &Card) -> bool {
        let power = self.purchasing_power(true);
        let mut missing = 0u8;
        for (i, &cost) in card.cost.iter().enumerate() {
            missing += cost.saturating_sub(power[i]);
        }
        self.tokens[5] >= missing
    }
    pub fn buy(&mut self, card: Card, bank: &mut [u8; 6]) {
        let card_power = self.purchasing_power(false);
        for (i, &cost) in card.cost.iter().enumerate() {
            let token_cost = cost.saturating_sub(card_power[i]);
            let missing = token_cost.saturating_sub(self.tokens[i]);
            if missing > 0 {
                bank[5] += missing;
                self.tokens[5] -= missing;
                bank[i] += self.tokens[i];
                self.tokens[i] = 0;
            } else {
                bank[i] += token_cost;
                self.tokens[i] -= token_cost;
            }
        }
        self.owned[card.color as usize].push(card.vp);
    }
    pub fn can_acquire(&self, noble: &Noble) -> bool {
        let power = self.purchasing_power(false);
        noble.cost.iter().zip(power.iter()).all(|(&c, &p)| c <= p)
    }
    pub fn acquire_best_noble(&mut self, all_nobles: &mut Vec<Noble>) -> u8 {
        let best_noble = all_nobles
            .iter()
            .enumerate()
            .filter(|(_, n)| self.can_acquire(n))
            .max_by_key(|(_, n)| n.vp);
        if let Some((idx, n)) = best_noble {
            let vp = n.vp;
            self.nobles.push(all_nobles.remove(idx));
            vp
        } else {
            0
        }
    }
    pub fn can_reserve(&self) -> bool {
        self.reserved.len() < 3
    }
    pub fn peek_reserved(&self, index: usize) -> Option<&Card> {
        self.reserved.get(index)
    }
    pub fn pop_reserved(&mut self, index: usize) -> Option<Card> {
        if index >= self.reserved.len() {
            return None;
        }
        Some(self.reserved.remove(index))
    }
    pub fn reserve(&mut self, card: Card, bank_gold: &mut u8) {
        self.reserved.push(card);
        if *bank_gold > 0 && self.num_tokens() < 10 {
            *bank_gold -= 1;
            self.tokens[5] += 1;
        }
    }
    pub fn buyable_reserved_cards(&self) -> Vec<usize> {
        self.reserved
            .iter()
            .enumerate()
            .filter(|(_, c)| self.can_buy(c))
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_types::Color;

    #[test]
    fn default() {
        let p = Player::default();
        assert_eq!(p.tokens, [0, 0, 0, 0, 0, 0]);
        assert_eq!(p.owned[0].len(), 0);
        assert_eq!(p.nobles.len(), 0);
        assert_eq!(p.vp(), 0);
        assert_eq!(p.purchasing_power(true), [0, 0, 0, 0, 0]);
        assert_eq!(p.purchasing_power(false), [0, 0, 0, 0, 0]);
    }

    #[test]
    fn can_buy() {
        let card = Card {
            level: 1,
            color: Color::White,
            vp: 1,
            cost: [1, 0, 0, 2, 0],
        };
        let mut p = Player::default();
        assert!(!p.can_buy(&card));
        p.tokens[0] = 1;
        assert!(!p.can_buy(&card));
        p.tokens[5] = 1;
        assert!(!p.can_buy(&card));
        p.tokens[1] = 1;
        assert!(!p.can_buy(&card));
        p.tokens[3] = 1;
        assert!(p.can_buy(&card));
        p.tokens[5] = 0;
        assert!(!p.can_buy(&card));
        p.tokens[3] = 4;
        assert!(p.can_buy(&card));
        p.tokens[0] = 0;
        assert!(!p.can_buy(&card));
        p.owned[0].push(1);
        assert!(p.can_buy(&card));
    }

    #[test]
    fn reserving() {
        // Initial state.
        let mut p = Player::default();
        assert!(p.can_reserve());
        assert!(p.peek_reserved(0).is_none());
        assert!(p.pop_reserved(0).is_none());
        assert_eq!(p.buyable_reserved_cards(), vec![]);

        // Reserve a card.
        let card = Card {
            level: 1,
            color: Color::White,
            vp: 1,
            cost: [2, 2, 0, 0, 0],
        };
        let mut bank_gold = 3;
        p.reserve(card, &mut bank_gold);
        assert_eq!(p.tokens[5], 1);
        assert_eq!(bank_gold, 2);
        assert!(p.can_reserve());
        assert_eq!(p.peek_reserved(0).unwrap().color, Color::White);
        assert_eq!(p.buyable_reserved_cards(), vec![]);

        // Reserve a second card, this time with no bank gold available.
        let card = Card {
            level: 1,
            color: Color::Blue,
            vp: 1,
            cost: [0, 1, 0, 0, 0],
        };
        bank_gold = 0;
        p.reserve(card, &mut bank_gold);
        assert_eq!(p.tokens[5], 1);
        assert_eq!(bank_gold, 0);
        assert!(p.can_reserve());
        assert_eq!(p.peek_reserved(1).unwrap().color, Color::Blue);
        assert_eq!(p.buyable_reserved_cards(), vec![1]);

        // Reserve a third card.
        let card = Card {
            level: 1,
            color: Color::Green,
            vp: 1,
            cost: [0, 0, 1, 1, 0],
        };
        bank_gold = 5;
        p.reserve(card, &mut bank_gold);
        assert_eq!(p.tokens[5], 2);
        assert_eq!(bank_gold, 4);
        assert!(!p.can_reserve());
        assert_eq!(p.peek_reserved(2).unwrap().color, Color::Green);
        assert_eq!(p.buyable_reserved_cards(), vec![1, 2]);

        // Remove the second card.
        assert_eq!(p.pop_reserved(1).unwrap().color, Color::Blue);
        assert_eq!(p.tokens[5], 2);
        assert!(p.can_reserve());
        assert_eq!(p.peek_reserved(0).unwrap().color, Color::White);
        assert_eq!(p.peek_reserved(1).unwrap().color, Color::Green);
        assert!(p.peek_reserved(2).is_none());
        assert_eq!(p.buyable_reserved_cards(), vec![1]);
    }
}
