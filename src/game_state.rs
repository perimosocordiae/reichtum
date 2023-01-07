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
        })
    }
    pub fn take_turn(&mut self, action: &Action) -> Result<bool, DynError> {
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
                let player = &mut self.players[self.curr_player_idx];
                if player.num_tokens() as usize + colors.len() > 10 {
                    return Err("Cannot take more than 10 tokens".into());
                }
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
                let player = &mut self.players[self.curr_player_idx];
                if player.num_tokens() + 2 > 10 {
                    return Err("Cannot take more than 10 tokens".into());
                }
                self.bank[c] -= 2;
                player.tokens[c] += 2;
            }
            Action::ReserveCard(loc) => {
                if let CardLocation::Reserve(_) = loc {
                    return Err("Card is already reserved".into());
                }
                let player = &self.players[self.curr_player_idx];
                if player.reserved.len() >= 3 {
                    return Err("At most 3 cards can be reserved".into());
                }
                let card = self.take_card(loc)?;
                let player = &mut self.players[self.curr_player_idx];
                player.reserved.push(card);
                if self.bank[5] > 0 && player.num_tokens() < 10 {
                    self.bank[5] -= 1;
                    player.tokens[5] += 1;
                }
            }
            Action::BuyCard(loc) => {
                if !self.players[self.curr_player_idx].can_buy(self.peek_card(loc)?) {
                    return Err("Cannot afford card".into());
                }
                let card = self.take_card(loc)?;
                let player = &mut self.players[self.curr_player_idx];
                let card_power = player.purchasing_power(false);
                for (i, &cost) in card.cost.iter().enumerate() {
                    let token_cost = cost.saturating_sub(card_power[i]);
                    let missing = token_cost.saturating_sub(player.tokens[i]);
                    if missing > 0 {
                        self.bank[5] += missing;
                        player.tokens[5] -= missing;
                        self.bank[i] += player.tokens[i];
                        player.tokens[i] = 0;
                    } else {
                        self.bank[i] += token_cost;
                        player.tokens[i] -= token_cost;
                    }
                }
                player.owned[card.color as usize].push(card.vp);
            }
        }
        // If a player can acquire a noble, they do so.
        // Only one noble is acquired per turn.
        let player = &mut self.players[self.curr_player_idx];
        let mut acquirable_nobles = self
            .nobles
            .iter()
            .enumerate()
            .filter(|(_, n)| player.can_acquire(n));
        if let Some((idx, _)) = acquirable_nobles.next() {
            player.nobles.push(self.nobles.remove(idx));
        }
        // Check for game over.
        if player.vp() >= 15 {
            // Invalid player index indicates that the game is over.
            self.curr_player_idx = self.players.len();
            return Ok(true);
        }
        // Advance to the next player.
        self.curr_player_idx = (self.curr_player_idx + 1) % self.players.len();
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
                .players
                .get(self.curr_player_idx)
                .ok_or("Invalid player")?
                .reserved
                .get(*idx)
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
            CardLocation::Reserve(idx) => {
                let player = &mut self.players[self.curr_player_idx];
                if !(0..player.reserved.len()).contains(idx) {
                    return Err("Invalid reservation".into());
                }
                Ok(player.reserved.remove(*idx))
            }
        }
    }
    pub fn valid_actions(&self) -> Vec<Action> {
        let mut actions = Vec::new();
        let player = &self.players[self.curr_player_idx];
        // Try to buy every available card in the market.
        for (level, market) in self.market.iter().enumerate() {
            for (idx, card) in market.iter().enumerate() {
                if player.can_buy(card) {
                    actions.push(Action::BuyCard(CardLocation::Market(level + 1, idx)));
                }
            }
        }
        // Try to buy every reserved card.
        for (idx, card) in player.reserved.iter().enumerate() {
            if player.can_buy(card) {
                actions.push(Action::BuyCard(CardLocation::Reserve(idx)));
            }
        }

        // Reserve every available card (including piles) if we have fewer than
        // 3 reserved already.
        if player.reserved.len() < 3 {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    TakeDifferentColorTokens(Vec<Color>),
    TakeSameColorTokens(Color),
    ReserveCard(CardLocation),
    BuyCard(CardLocation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CardLocation {
    Pile(usize),
    Market(usize, usize),
    Reserve(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    level: usize,
    // Production color
    pub color: Color,
    // Victory points
    pub vp: u8,
    // Cost to buy this card: [white, blue, green, red, black]
    pub cost: [u8; 5],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Noble {
    // Victory points
    pub vp: u8,
    // Cost to acquire: [white, blue, green, red, black]
    pub cost: [u8; 5],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    // Token counts: [white, blue, green, red, black, gold]
    tokens: [u8; 6],
    // Purchased cards: [white, blue, green, red, black]
    owned: [Vec<u8>; 5],
    // Reserved cards
    reserved: Vec<Card>,
    // Acquired nobles
    nobles: Vec<Noble>,
}
impl Player {
    fn default() -> Self {
        Self {
            tokens: [0, 0, 0, 0, 0, 0],
            owned: [Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            reserved: Vec::new(),
            nobles: Vec::new(),
        }
    }
    pub fn num_tokens(&self) -> u8 {
        self.tokens.iter().sum()
    }
    pub fn vp(&self) -> u8 {
        self.owned.iter().map(|c| c.iter().sum::<u8>()).sum::<u8>()
            + self.nobles.iter().map(|n| n.vp).sum::<u8>()
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
    fn can_buy(&self, card: &Card) -> bool {
        let power = self.purchasing_power(true);
        let mut missing = 0u8;
        for (i, &cost) in card.cost.iter().enumerate() {
            missing += cost.saturating_sub(power[i]);
        }
        self.tokens[5] >= missing
    }
    fn can_acquire(&self, noble: &Noble) -> bool {
        let power = self.purchasing_power(false);
        noble.cost.iter().zip(power.iter()).all(|(&c, &p)| c <= p)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    White,
    Blue,
    Green,
    Red,
    Black,
    Gold,
}
impl TryFrom<usize> for Color {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Color::White),
            1 => Ok(Color::Blue),
            2 => Ok(Color::Green),
            3 => Ok(Color::Red),
            4 => Ok(Color::Black),
            5 => Ok(Color::Gold),
            _ => Err(()),
        }
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

#[test]
fn test_load_cards_from_csv() {
    let cards = load_from_csv::<Card>(
        "level,color,vp,cost\n\
         1,black,0,1,1,1,1,0\n\
         1,black,0,1,2,1,1,0",
    )
    .unwrap();
    assert_eq!(cards.len(), 2);
}

#[test]
fn test_load_nobles_from_csv() {
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
fn test_new_player() {
    let p = Player::default();
    assert_eq!(p.tokens, [0, 0, 0, 0, 0, 0]);
    assert_eq!(p.owned[0].len(), 0);
    assert_eq!(p.nobles.len(), 0);
    assert_eq!(p.vp(), 0);
    assert_eq!(p.purchasing_power(true), [0, 0, 0, 0, 0]);
    assert_eq!(p.purchasing_power(false), [0, 0, 0, 0, 0]);
}

#[test]
fn test_player_can_buy() {
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
fn test_game_state_init() {
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
fn test_game_turns() {
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
