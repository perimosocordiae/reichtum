use crate::data_types::{Action, CardLocation};
use crate::game_state::GameState;
use rand::seq::IndexedRandom;

pub fn create_agent(difficulty: usize) -> Box<dyn Agent + Send> {
    match difficulty {
        // Completely random actions.
        0 => Box::<RandomAgent>::default(),
        // Only cares about VP.
        1 => Box::new(GreedyAgent {
            bonuses: ScoringBonuses {
                vp: 100,
                card_needed: 0,
                color_needed: 0,
                reserve_discount: 10,
            },
        }),
        // Balances raw VP, nobles, and card purchasing power.
        _ => Box::new(GreedyAgent {
            bonuses: ScoringBonuses {
                vp: 1000,
                card_needed: 10,
                color_needed: 1,
                reserve_discount: 10,
            },
        }),
    }
}

pub trait Agent {
    fn choose_action(&self, game: &GameState) -> Action;
}

#[derive(Default)]
pub struct RandomAgent;
impl Agent for RandomAgent {
    fn choose_action(&self, game: &GameState) -> Action {
        let mut rng = rand::rng();
        let actions = game.valid_actions();
        if let Some(m) = actions.choose(&mut rng) {
            m.clone()
        } else {
            panic!("No moves to choose from! GameState: {:?}", game);
        }
    }
}

pub struct GreedyAgent {
    bonuses: ScoringBonuses,
}
impl Agent for GreedyAgent {
    fn choose_action(&self, game: &GameState) -> Action {
        let actions = game.valid_actions();
        if actions.len() == 1 {
            return actions[0].clone();
        }
        let info = ScoringInfo::new(game);
        let scored_actions = actions
            .iter()
            .map(|a| (a, info.score_action(game, a, &self.bonuses)))
            .collect::<Vec<_>>();
        let best_score = scored_actions.iter().map(|(_, s)| s).max().unwrap();
        let best_actions: Vec<&Action> = scored_actions
            .iter()
            .filter(|(_, s)| s == best_score)
            .map(|(a, _)| *a)
            .collect();
        let mut rng = rand::rng();
        let best = best_actions.choose(&mut rng).unwrap();
        (*best).clone()
    }
}

struct ScoringBonuses {
    vp: i32,
    card_needed: i32,
    color_needed: i32,
    reserve_discount: i32,
}

struct ScoringInfo {
    // Max cards needed for noble acquisition.
    cards_needed: [i32; 5],
    // Count of token colors needed (excluding gold) for card purchasing.
    colors_needed: [i32; 5],
}
impl ScoringInfo {
    fn new(game: &GameState) -> Self {
        let me = game.curr_player();
        let cards = me.purchasing_power(false);
        let mut cards_needed = [0, 0, 0, 0, 0];
        for n in game.nobles.iter() {
            for (i, c) in n.cost.iter().enumerate() {
                if c > &cards[i] {
                    cards_needed[i] = std::cmp::max(cards_needed[i], (c - cards[i]) as i32);
                }
            }
        }
        let power = me.purchasing_power(true);
        let mut colors_needed = [0, 0, 0, 0, 0];
        for row in game.market.iter() {
            for card in row.iter() {
                for (i, c) in card.cost.iter().enumerate() {
                    if c > &power[i] {
                        colors_needed[i] += 1;
                    }
                }
            }
        }
        Self {
            cards_needed,
            colors_needed,
        }
    }

    fn score_action(&self, game: &GameState, action: &Action, bonuses: &ScoringBonuses) -> i32 {
        match action {
            Action::TakeDifferentColorTokens(colors) => colors
                .iter()
                .map(|c| self.colors_needed[*c as usize] * bonuses.color_needed)
                .sum(),
            Action::TakeSameColorTokens(color) => {
                self.colors_needed[*color as usize] * bonuses.color_needed
            }
            Action::BuyCard(loc) => {
                let card = game.peek_card(loc).unwrap();
                // Prefer cards in the reserve, but only a tiny bit.
                let loc_bonus = match loc {
                    CardLocation::Reserve(_) => 1,
                    _ => 0,
                };
                let idx = card.color as usize;
                card.vp as i32 * bonuses.vp
                    + self.cards_needed[idx] * bonuses.card_needed
                    + self.colors_needed[idx] * bonuses.color_needed
                    + loc_bonus
            }
            Action::ReserveCard(loc) => {
                if let Ok(card) = game.peek_card(loc) {
                    let idx = card.color as usize;
                    (card.vp as i32 * bonuses.vp + self.cards_needed[idx] * bonuses.card_needed)
                        / bonuses.reserve_discount
                } else {
                    // Reserving from the pile is almost never a good idea.
                    -1
                }
            }
        }
    }
}
