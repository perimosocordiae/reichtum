use crate::data_types::{Action, CardLocation};
use crate::game_state::GameState;
use rand::seq::IndexedRandom;

pub fn create_agent(difficulty: usize) -> Box<dyn Agent + Send> {
    match difficulty {
        // Completely random actions.
        0 => Box::<RandomAgent>::default(),
        // Weak Greedy Agent (only looks at raw VP gain).
        1 => Box::new(GreedyAgent {
            bonuses: ScoringBonuses {
                vp: 100,
                card_needed: 0,
                color_needed: 0,
                reserve_discount: 10,
            },
        }),
        // Strong Greedy Agent (prioritizes progress toward cards + nobles).
        2 => Box::new(GreedyAgent {
            bonuses: ScoringBonuses {
                vp: 1000,
                card_needed: 10,
                color_needed: 1,
                reserve_discount: 10,
            },
        }),
        // Smart Agent (strong greedy + extra heuristics).
        _ => Box::new(SmartAgent),
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
    // TODO: add `spend_cost` here such that, when it's > 0, scoring will
    // prefer buying cards that require spending fewer tokens, esp. gold.
    // Then introduce a new GreedyAgent difficulty level (3) that uses it to
    // measure how much it helps.
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

pub struct SmartAgent;

impl Agent for SmartAgent {
    fn choose_action(&self, game: &GameState) -> Action {
        let actions = game.valid_actions();
        if actions.len() == 1 {
            return actions[0].clone();
        }

        let me = game.curr_player();
        let my_vp = me.vp() as i32;
        let gems = me.purchasing_power(false);

        // 1. Check for Winning Move (BuyCard that reaches 15 VP, including nobles)
        for action in &actions {
            if let Action::BuyCard(loc) = action {
                let card = game.peek_card(loc).unwrap();
                let card_vp = card.vp as i32;

                // Calculate if this triggers a noble
                let mut noble_vp = 0;
                let card_color = card.color as usize;
                for noble in &game.nobles {
                     // Check if we meet requirements AFTER buying this card
                     // We need noble.cost <= current gems + new card
                     let mut meets = true;
                     for (i, &cost) in noble.cost.iter().enumerate() {
                         let my_gem_count = gems[i] + if i == card_color { 1 } else { 0 };
                         if my_gem_count < cost {
                             meets = false;
                             break;
                         }
                     }
                     // Also, we must not ALREADY have this noble (but game.nobles only contains available ones)
                     if meets {
                         noble_vp += noble.vp as i32;
                     }
                }

                if my_vp + card_vp + noble_vp >= 15 {
                    return action.clone();
                }
            }
        }

        // 2. Fallback to Strong Greedy Strategy (d=2)
        // But with slight bias for Noble proximity (Greedy doesn't see Noble proximity well)

        let bonuses = ScoringBonuses {
            vp: 1000,
            card_needed: 10,
            color_needed: 1,
            reserve_discount: 10,
        };
        let info = ScoringInfo::new(game);

        let mut best_action = &actions[0];
        let mut best_score = i32::MIN;

        for action in &actions {
            let mut score = info.score_action(game, action, &bonuses);

            // Add Smart Heuristics on top of Greedy Score

            if let Action::BuyCard(loc) = action {
                let card = game.peek_card(loc).unwrap();
                let card_color = card.color as usize;

                // Bonus for getting CLOSER to a noble (Greedy only cares about linear distance)
                // We reward being 1 turn away.
                for noble in &game.nobles {
                    let needed = noble.cost[card_color].saturating_sub(gems[card_color]);
                    if needed > 0 {
                         // This card helps.
                         // Calculate remaining distance for ALL colors
                         let mut dist = 0;
                         for (i, &cost) in noble.cost.iter().enumerate() {
                             let my_count = gems[i] + if i == card_color { 1 } else { 0 };
                             dist += cost.saturating_sub(my_count);
                         }

                         if dist == 0 {
                             score += 2500; // Triggers noble (less than 15 VP win, but huge)
                             // Note: Greedy sees 3 VP noble? No, Greedy doesn't see noble trigger in score_action.
                             // acquire_best_noble is in take_turn.
                             // So Greedy totally misses that a card triggers a noble!
                         } else if dist <= 1 {
                             score += 500; // Almost there
                         }
                    }
                }
            }

            // Penalty for hoarding tokens (Greedy doesn't care)
            if let Action::TakeDifferentColorTokens(_) | Action::TakeSameColorTokens(_) = action {
                if me.num_tokens() >= 8 {
                    score -= 200;
                }
            }

            if score > best_score {
                best_score = score;
                best_action = action;
            }
        }

        // Random tie-break
        // (Simplified: just take first best)

        best_action.clone()
    }
}
