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
        // Smart Agent
        2 => Box::new(SmartAgent),
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

pub struct SmartAgent;

impl Agent for SmartAgent {
    fn choose_action(&self, game: &GameState) -> Action {
        let actions = game.valid_actions();
        if actions.len() == 1 {
            return actions[0].clone();
        }

        let me = game.curr_player();
        let power = me.purchasing_power(true);
        let gems = me.purchasing_power(false);
        let my_vp = me.vp();

        // Calculate desire for each color
        let mut color_weights = [0.0; 5]; // Default 0

        let mut potential_cards = Vec::new();
        for (r, row) in game.market.iter().enumerate() {
             for (c, card) in row.iter().enumerate() {
                 potential_cards.push((card, CardLocation::Market(r + 1, c)));
             }
        }
        for i in 0..me.reserved.len() {
             if let Some(card) = me.peek_reserved(i) {
                 potential_cards.push((card, CardLocation::Reserve(i)));
             }
        }

        for (card, _loc) in &potential_cards {
            let mut missing_count = 0;
            let mut missing_colors = [0; 5];

            for (i, &cost) in card.cost.iter().enumerate() {
                if cost > power[i] {
                    let needed = cost - power[i];
                    missing_count += needed;
                    missing_colors[i] = needed;
                }
            }

            // Filter: Ignore impossible cards (e.g. need 5+ tokens)
            // Stricter filter
            if missing_count > 5 {
                continue;
            }

            // Value Calculation
            let mut value = 0.0;

            // VP Value - Increased significance
            // 1 VP is worth a lot.
            value += (card.vp as f32) * 25.0;

            // Noble Value
            let card_color = card.color as usize;
            let mut noble_bonus = 0.0;
            for noble in &game.nobles {
                 if gems[card_color] < noble.cost[card_color] {
                     // We need this color for noble.
                     // How close are we?
                     let dist: u8 = noble.cost.iter().zip(gems.iter()).map(|(c, p)| c.saturating_sub(*p)).sum();

                     // If buying this card makes noble reachable immediately or very soon
                     // Noble is 3 points. That's worth ~75 score points in my VP scale.
                     if dist <= 1 {
                        noble_bonus += 40.0;
                     } else if dist <= 3 {
                        noble_bonus += 20.0;
                     } else {
                        noble_bonus += 5.0;
                     }
                 }
            }
            value += noble_bonus;

            // Engine building value
            let total_gems: u8 = gems.iter().sum();
            if total_gems < 8 {
                value += 10.0; // Early game build engine
            } else {
                value += 2.0;
            }

            // Discount by distance
            let factor = 1.0 / (missing_count as f32 + 1.0);
            let adjusted_value = value * factor;

            if missing_count > 0 {
                for i in 0..5 {
                    if missing_colors[i] > 0 {
                        color_weights[i] += adjusted_value;
                    }
                }
            }
        }

        let mut best_action = &actions[0];
        let mut best_score = f32::NEG_INFINITY;

        for action in &actions {
            let score = match action {
                Action::TakeDifferentColorTokens(colors) => {
                    let mut s = 0.0;
                    for &c in colors {
                        s += color_weights[c as usize];
                    }
                    if s == 0.0 {
                        s = -5.0;
                    }
                    // Penalize hoarding
                    let num_tokens = me.num_tokens();
                    if num_tokens > 8 {
                        s -= 20.0;
                    } else if num_tokens > 6 {
                        s -= 5.0;
                    }
                    s
                },
                Action::TakeSameColorTokens(color) => {
                    let w = color_weights[*color as usize];
                    if w > 0.0 {
                        w * 2.5
                    } else {
                        -5.0
                    }
                },
                Action::BuyCard(loc) => {
                    let card = game.peek_card(loc).unwrap();
                    let mut val = 0.0;

                    // WINNING MOVE CHECK
                    if my_vp as u16 + card.vp as u16 >= 15 {
                        val += 10000.0;
                    }

                    val += (card.vp as f32) * 30.0;

                    let card_color = card.color as usize;
                    for noble in &game.nobles {
                         if gems[card_color] < noble.cost[card_color] {
                             let dist: u8 = noble.cost.iter().zip(gems.iter()).map(|(c, p)| c.saturating_sub(*p)).sum();
                             if dist == 0 {
                                 // Buying this card triggers noble!
                                 // Note: acquire_best_noble happens after turn.
                                 // So we will get noble.
                                 val += 100.0; // Huge bonus (3 VP)
                                 if my_vp as u16 + card.vp as u16 + 3 >= 15 {
                                     val += 10000.0;
                                 }
                             } else if dist <= 1 {
                                val += 40.0;
                             } else if dist <= 3 {
                                val += 20.0;
                             } else {
                                val += 5.0;
                             }
                         }
                    }

                    let total_gems: u8 = gems.iter().sum();
                    if total_gems < 8 {
                        val += 15.0;
                    } else {
                        val += 5.0;
                    }

                    if let CardLocation::Reserve(_) = loc {
                        val += 10.0;
                    }

                    val += 25.0; // Action bias

                    val
                },
                Action::ReserveCard(loc) => {
                     if let Ok(card) = game.peek_card(loc) {
                         let mut val = -15.0; // Higher penalty

                         if card.vp >= 4 {
                             val += 40.0;
                         } else if card.vp == 3 {
                             val += 20.0;
                         }

                         if game.bank[5] > 0 && me.num_tokens() < 10 {
                             val += 10.0;
                         }

                         // Lookahead: if we can almost buy it?
                         let mut missing = 0;
                         for (i, &cost) in card.cost.iter().enumerate() {
                            if cost > power[i] {
                                missing += cost - power[i];
                            }
                         }
                         if missing <= 2 && card.vp >= 3 {
                             val += 10.0; // Secure it
                         }

                         val
                     } else {
                         -100.0
                     }
                }
            };

            // Tiny noise to break ties randomly
            // score += rng.random::<f32>() * 0.01;

            if score > best_score {
                best_score = score;
                best_action = action;
            }
        }

        best_action.clone()
    }
}
