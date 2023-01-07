use crate::data_types::{Action, Card, CardLocation, Color, Noble};
use crate::player::Player;
use rand::{prelude::SliceRandom, seq::IteratorRandom};
use serde::{Deserialize, Serialize};

type DynError = Box<dyn std::error::Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    // 3 piles of cards, one per level, face down.
    #[serde(skip)]
    piles: [Vec<Card>; 3],

    // 3 rows of buyable cards, 4 per level, face up.
    pub market: [Vec<Card>; 3],

    // Available nobles, face up.
    pub nobles: Vec<Noble>,

    // Token bank: [white, blue, green, red, black, gold]
    pub bank: [u8; 6],

    // Players.
    pub players: Vec<Player>,
    pub curr_player_idx: usize,

    // Current round number.
    round: u16,
}
impl GameState {
    pub fn init(num_players: usize) -> Result<GameState, DynError> {
        if !(2..=9).contains(&num_players) {
            return Err("Invalid number of players".into());
        }
        let cards = load_from_csv::<Card>(include_str!("../cards.csv"))?;
        let mut market = [Vec::new(), Vec::new(), Vec::new()];
        for card in cards {
            market[card.level - 1].push(card);
        }
        let mut rng = rand::thread_rng();
        market[0].shuffle(&mut rng);
        market[1].shuffle(&mut rng);
        market[2].shuffle(&mut rng);
        let piles = [
            market[0].split_off(4),
            market[1].split_off(4),
            market[2].split_off(4),
        ];

        let mut nobles = load_from_csv::<Noble>(include_str!("../nobles.csv"))?;
        nobles.shuffle(&mut rng);
        nobles.truncate(num_players + 1);

        let curr_player_idx = (0..num_players).choose(&mut rng).unwrap_or(0);

        let bank = match num_players {
            2 => [4, 4, 4, 4, 4, 5],
            3 => [5, 5, 5, 5, 5, 5],
            _ => [7, 7, 7, 7, 7, 5],
        };

        Ok(GameState {
            piles,
            market,
            nobles,
            bank,
            players: (0..num_players).map(|_| Player::default()).collect(),
            curr_player_idx,
            round: 1,
        })
    }
    fn curr_player(&self) -> &Player {
        &self.players[self.curr_player_idx]
    }
    pub fn take_turn(&mut self, action: &Action) -> Result<bool, DynError> {
        let old_vp = self.curr_player().vp();
        let mut new_vp = old_vp;
        match action {
            Action::TakeDifferentColorTokens(colors) => {
                if colors.len() > 3 {
                    return Err("Cannot take more than 3 tokens".into());
                }
                for (i, &c) in colors.iter().enumerate() {
                    if c == Color::Gold {
                        return Err("Cannot take a gold token".into());
                    }
                    if self.bank[c as usize] == 0 {
                        return Err("Not enough tokens in bank".into());
                    }
                    if colors[i + 1..].contains(&c) {
                        return Err("Cannot take the same color twice".into());
                    }
                }
                if self.curr_player().num_tokens() as usize + colors.len() > 10 {
                    return Err("Cannot take more than 10 tokens".into());
                }
                let player = &mut self.players[self.curr_player_idx];
                for &color in colors {
                    let c = color as usize;
                    if self.bank[c] > 0 {
                        self.bank[c] -= 1;
                        player.tokens[c] += 1;
                    }
                }
            }
            Action::TakeSameColorTokens(color) => {
                if color == &Color::Gold {
                    return Err("Cannot take a gold token".into());
                }
                let c = *color as usize;
                if self.bank[c] < 4 {
                    return Err("Not enough tokens in bank".into());
                }
                if self.curr_player().num_tokens() + 2 > 10 {
                    return Err("Cannot take more than 10 tokens".into());
                }
                self.bank[c] -= 2;
                self.players[self.curr_player_idx].tokens[c] += 2;
            }
            Action::ReserveCard(loc) => {
                if let CardLocation::Reserve(_) = loc {
                    return Err("Card is already reserved".into());
                }
                if !self.curr_player().can_reserve() {
                    return Err("At most 3 cards can be reserved".into());
                }
                let card = self.take_card(loc)?;
                self.players[self.curr_player_idx].reserve(card, &mut self.bank[5]);
            }
            Action::BuyCard(loc) => {
                if !self.curr_player().can_buy(self.peek_card(loc)?) {
                    return Err("Cannot afford card".into());
                }
                let card = self.take_card(loc)?;
                new_vp += card.vp;
                self.players[self.curr_player_idx].buy(card, &mut self.bank);
            }
        }
        // If a player can acquire a noble, they do so.
        // At most one noble can be acquired per player per round.
        new_vp += self.players[self.curr_player_idx].acquire_best_noble(&mut self.nobles);
        // Update the player's VP history, if they gained VP.
        if new_vp > old_vp {
            self.players[self.curr_player_idx]
                .vp_history
                .push((self.round, new_vp));
        }
        // Advance to the next player.
        self.curr_player_idx += 1;
        // If the round is over, check if the game is over too.
        if self.curr_player_idx == self.players.len() {
            // If any player has 15+ VP, the game is over.
            if self.players.iter().any(|p| p.vp() >= 15) {
                return Ok(true);
            }
            self.round += 1;
            self.curr_player_idx = 0;
        }
        Ok(false)
    }
    pub fn is_finished(&self) -> bool {
        self.curr_player_idx >= self.players.len()
    }
    pub fn peek_card(&self, loc: &CardLocation) -> Result<&Card, DynError> {
        match loc {
            CardLocation::Pile(_) => Err("No peeking at the pile".into()),
            CardLocation::Market(level, idx) => self
                .market
                .get(*level - 1)
                .ok_or("Invalid market level")?
                .get(*idx)
                .ok_or_else(|| "Invalid market index".into()),
            CardLocation::Reserve(idx) => self
                .curr_player()
                .peek_reserved(*idx)
                .ok_or_else(|| "Invalid reserve index".into()),
        }
    }
    fn take_card(&mut self, loc: &CardLocation) -> Result<Card, DynError> {
        match loc {
            CardLocation::Pile(level) => {
                if !(1..=3).contains(level) {
                    return Err("Invalid pile level".into());
                }
                self.piles[*level - 1]
                    .pop()
                    .ok_or_else(|| "No cards left".into())
            }
            CardLocation::Market(level, idx) => {
                if !(1..=3).contains(level) {
                    return Err("Invalid market level".into());
                }
                let pile = &mut self.piles[*level - 1];
                let market = &mut self.market[*level - 1];
                if !(0..market.len()).contains(idx) {
                    return Err("Invalid market index".into());
                }
                Ok(if pile.is_empty() {
                    market.remove(*idx)
                } else {
                    market.push(pile.pop().unwrap());
                    market.swap_remove(*idx)
                })
            }
            CardLocation::Reserve(idx) => self.players[self.curr_player_idx]
                .pop_reserved(*idx)
                .ok_or_else(|| "Invalid reserve index".into()),
        }
    }
    pub fn valid_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        let player = self.curr_player();
        // Try to buy every available card in the market.
        for (level, market) in self.market.iter().enumerate() {
            for (idx, card) in market.iter().enumerate() {
                if player.can_buy(card) {
                    actions.push(Action::BuyCard(CardLocation::Market(level + 1, idx)));
                }
            }
        }
        // Try to buy every reserved card.
        for idx in player.buyable_reserved_cards() {
            actions.push(Action::BuyCard(CardLocation::Reserve(idx)));
        }

        // Reserve every available card (including piles) if we have fewer than
        // 3 reserved already.
        if player.can_reserve() {
            for (level, market) in self.market.iter().enumerate() {
                for idx in 0..market.len() {
                    actions.push(Action::ReserveCard(CardLocation::Market(level + 1, idx)));
                }
                if !self.piles[level].is_empty() {
                    actions.push(Action::ReserveCard(CardLocation::Pile(level + 1)));
                }
            }
        }

        // Take tokens from the bank, if possible.
        let num_tokens = player.num_tokens();
        if num_tokens <= 8 {
            for i in 0..5 {
                if self.bank[i] >= 4 {
                    actions.push(Action::TakeSameColorTokens(i.try_into().unwrap()));
                }
            }
        }
        // Take up to 3 different color tokens, if possible.
        let prev_num_actions = actions.len();
        if num_tokens <= 7 {
            for i in 0..3 {
                if self.bank[i] == 0 {
                    continue;
                }
                for j in i + 1..4 {
                    if self.bank[j] == 0 {
                        continue;
                    }
                    for k in j + 1..5 {
                        if self.bank[k] > 0 {
                            actions.push(Action::TakeDifferentColorTokens(vec![
                                i.try_into().unwrap(),
                                j.try_into().unwrap(),
                                k.try_into().unwrap(),
                            ]));
                        }
                    }
                }
            }
        }
        // Only take two different color tokens if we can't take three.
        if num_tokens <= 8 && actions.len() == prev_num_actions {
            for i in 0..4 {
                if self.bank[i] == 0 {
                    continue;
                }
                for j in i + 1..5 {
                    if self.bank[j] > 0 {
                        actions.push(Action::TakeDifferentColorTokens(vec![
                            i.try_into().unwrap(),
                            j.try_into().unwrap(),
                        ]));
                    }
                }
            }
        }
        // Only take one single token if we can't take two.
        if num_tokens <= 9 && actions.len() == prev_num_actions {
            for i in 0..5 {
                if self.bank[i] > 0 {
                    actions.push(Action::TakeDifferentColorTokens(vec![i
                        .try_into()
                        .unwrap()]));
                }
            }
        }

        // As a last resort, do nothing.
        if actions.is_empty() {
            actions.push(Action::TakeDifferentColorTokens(vec![]));
        }

        actions
    }
}

fn load_from_csv<T: for<'de> Deserialize<'de>>(data: &str) -> Result<Vec<T>, DynError> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(data.as_bytes());
    let mut out = Vec::new();
    for result in rdr.deserialize::<T>() {
        let record: T = result?;
        out.push(record);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_cards_from_csv() {
        let cards = load_from_csv::<Card>(
            "level,color,vp,cost\n\
         1,black,0,1,1,1,1,0\n\
         1,black,0,1,2,1,1,0",
        )
        .unwrap();
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn load_nobles_from_csv() {
        let nobles = load_from_csv::<Noble>(
            "vp,cost\n\
        3,0,0,4,4,0\n\
        3,3,0,0,3,3\n\
        3,4,4,0,0,0",
        )
        .unwrap();
        assert_eq!(nobles.len(), 3);
    }

    #[test]
    fn init() {
        let gs = GameState::init(2).unwrap();
        assert_eq!(gs.piles[0].len(), 36);
        assert_eq!(gs.piles[1].len(), 26);
        assert_eq!(gs.piles[2].len(), 16);
        assert_eq!(gs.market[0].len(), 4);
        assert_eq!(gs.market[1].len(), 4);
        assert_eq!(gs.market[2].len(), 4);
        assert_eq!(gs.nobles.len(), 3);
    }

    #[test]
    fn game_turns() {
        let mut gs = GameState::init(2).unwrap();
        let starting_idx = gs.curr_player_idx;
        assert!(!gs
            .take_turn(&Action::TakeDifferentColorTokens(vec![
                Color::White,
                Color::Blue,
                Color::Green
            ]))
            .unwrap());
        assert_eq!(gs.players[starting_idx].num_tokens(), 3);
        let other_idx = gs.curr_player_idx;
        assert_ne!(other_idx, starting_idx);
        assert!(!gs
            .take_turn(&Action::TakeSameColorTokens(Color::Red))
            .unwrap());
        assert_eq!(gs.players[other_idx].num_tokens(), 2);
        assert_eq!(gs.curr_player_idx, starting_idx);
    }

    #[test]
    fn initial_valid_actions() {
        let gs = GameState::init(2).unwrap();
        assert_eq!(
            gs.valid_actions(),
            vec![
                Action::ReserveCard(CardLocation::Market(1, 0)),
                Action::ReserveCard(CardLocation::Market(1, 1)),
                Action::ReserveCard(CardLocation::Market(1, 2)),
                Action::ReserveCard(CardLocation::Market(1, 3)),
                Action::ReserveCard(CardLocation::Pile(1)),
                Action::ReserveCard(CardLocation::Market(2, 0)),
                Action::ReserveCard(CardLocation::Market(2, 1)),
                Action::ReserveCard(CardLocation::Market(2, 2)),
                Action::ReserveCard(CardLocation::Market(2, 3)),
                Action::ReserveCard(CardLocation::Pile(2)),
                Action::ReserveCard(CardLocation::Market(3, 0)),
                Action::ReserveCard(CardLocation::Market(3, 1)),
                Action::ReserveCard(CardLocation::Market(3, 2)),
                Action::ReserveCard(CardLocation::Market(3, 3)),
                Action::ReserveCard(CardLocation::Pile(3)),
                Action::TakeSameColorTokens(Color::White),
                Action::TakeSameColorTokens(Color::Blue),
                Action::TakeSameColorTokens(Color::Green),
                Action::TakeSameColorTokens(Color::Red),
                Action::TakeSameColorTokens(Color::Black),
                Action::TakeDifferentColorTokens(vec![Color::White, Color::Blue, Color::Green]),
                Action::TakeDifferentColorTokens(vec![Color::White, Color::Blue, Color::Red]),
                Action::TakeDifferentColorTokens(vec![Color::White, Color::Blue, Color::Black]),
                Action::TakeDifferentColorTokens(vec![Color::White, Color::Green, Color::Red]),
                Action::TakeDifferentColorTokens(vec![Color::White, Color::Green, Color::Black]),
                Action::TakeDifferentColorTokens(vec![Color::White, Color::Red, Color::Black]),
                Action::TakeDifferentColorTokens(vec![Color::Blue, Color::Green, Color::Red]),
                Action::TakeDifferentColorTokens(vec![Color::Blue, Color::Green, Color::Black]),
                Action::TakeDifferentColorTokens(vec![Color::Blue, Color::Red, Color::Black]),
                Action::TakeDifferentColorTokens(vec![Color::Green, Color::Red, Color::Black])
            ]
        );
    }

    #[test]
    fn no_valid_actions() {
        let mut gs = GameState::init(2).unwrap();
        // Remove all the cards from the market, so we can't buy any.
        gs.market[0].clear();
        gs.market[1].clear();
        gs.market[2].clear();

        {
            let mut player = &mut gs.players[gs.curr_player_idx];
            // Fill the player's token quota, so they can't take any more.
            player.tokens[0] = 10;
            // Fill the player's reserve, so they can't reserve any more.
            player.reserve(
                Card {
                    level: 1,
                    color: Color::White,
                    vp: 0,
                    cost: [1, 1, 1, 1, 0],
                },
                &mut gs.bank[5],
            );
            player.reserve(
                Card {
                    level: 1,
                    color: Color::Green,
                    vp: 0,
                    cost: [1, 1, 1, 1, 0],
                },
                &mut gs.bank[5],
            );
            player.reserve(
                Card {
                    level: 1,
                    color: Color::Blue,
                    vp: 0,
                    cost: [1, 1, 1, 1, 0],
                },
                &mut gs.bank[5],
            );
        }
        assert_eq!(
            gs.valid_actions(),
            vec![Action::TakeDifferentColorTokens(vec![])]
        );

        // If we have 9 tokens, we can take a single token of any available color.
        gs.players[gs.curr_player_idx].tokens[0] = 9;
        assert_eq!(
            gs.valid_actions(),
            vec![
                Action::TakeDifferentColorTokens(vec![Color::White]),
                Action::TakeDifferentColorTokens(vec![Color::Blue]),
                Action::TakeDifferentColorTokens(vec![Color::Green]),
                Action::TakeDifferentColorTokens(vec![Color::Red]),
                Action::TakeDifferentColorTokens(vec![Color::Black])
            ]
        );

        // Ensure we omit colors that have no tokens available in the bank.
        gs.bank[0] = 0;
        gs.bank[1] = 0;
        assert_eq!(
            gs.valid_actions(),
            vec![
                Action::TakeDifferentColorTokens(vec![Color::Green]),
                Action::TakeDifferentColorTokens(vec![Color::Red]),
                Action::TakeDifferentColorTokens(vec![Color::Black])
            ]
        );
    }
}
