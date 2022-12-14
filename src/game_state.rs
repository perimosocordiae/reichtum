use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use std::path::Path;

type DynError = Box<dyn std::error::Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    // 3 piles of cards, one per level, face down.
    #[serde(skip)]
    piles: [Vec<Card>; 3],

    // 3 rows of buyable cards, 4 per level, face up.
    market: [Vec<Card>; 3],

    // Available nobles, face up.
    nobles: Vec<Noble>,

    // Token bank: [white, blue, green, red, black, gold]
    bank: [u8; 6],

    // Players.
    players: Vec<Player>,
}
impl GameState {
    pub fn init(num_players: usize, data_dir: &Path) -> Result<GameState, DynError> {
        if !(2..=9).contains(&num_players) {
            return Err("Invalid number of players".into());
        }
        let cards = load_from_csv::<Card>(&data_dir.join("cards.csv"))?;
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

        let mut nobles = load_from_csv::<Noble>(&data_dir.join("nobles.csv"))?;
        nobles.shuffle(&mut rng);
        nobles.truncate(num_players + 1);

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
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Card {
    level: usize,
    // Production color
    color: Color,
    // Victory points
    vp: u8,
    // Cost to buy this card: [white, blue, green, red, black]
    cost: [u8; 5],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Noble {
    // Victory points
    vp: u8,
    // Cost to acquire: [white, blue, green, red, black]
    cost: [u8; 5],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Player {
    // Token counts: [white, blue, green, red, black, gold]
    tokens: [u8; 6],
    // Purchased cards
    cards: Vec<Card>,
    // Acquired nobles
    nobles: Vec<Noble>,
}
impl Player {
    fn default() -> Self {
        Self {
            tokens: [0, 0, 0, 0, 0, 0],
            cards: Vec::new(),
            nobles: Vec::new(),
        }
    }
    fn vp(&self) -> u8 {
        self.cards.iter().map(|c| c.vp).sum::<u8>() + self.nobles.iter().map(|n| n.vp).sum::<u8>()
    }
    fn purchasing_power(&self, include_tokens: bool) -> [u8; 5] {
        let mut power: [u8; 5] = [0, 0, 0, 0, 0];
        if include_tokens {
            power.copy_from_slice(&self.tokens[0..5]);
        }
        for card in &self.cards {
            power[card.color as usize] += 1;
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    White,
    Blue,
    Green,
    Red,
    Black,
    Gold,
}

fn load_from_csv<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, DynError> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_path(path)?;
    let mut out = Vec::new();
    for result in rdr.deserialize::<T>() {
        let record: T = result?;
        out.push(record);
    }
    Ok(out)
}

#[test]
fn test_load_cards_from_csv() {
    let cards = load_from_csv::<Card>(Path::new("cards.csv")).unwrap();
    assert_eq!(cards.len(), 90);
}

#[test]
fn test_load_nobles_from_csv() {
    let nobles = load_from_csv::<Noble>(Path::new("nobles.csv")).unwrap();
    assert_eq!(nobles.len(), 10);
}

#[test]
fn test_new_player() {
    let p = Player::default();
    assert_eq!(p.tokens, [0, 0, 0, 0, 0, 0]);
    assert_eq!(p.cards.len(), 0);
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
    p.cards.push(card.clone());
    assert!(p.can_buy(&card));
}

#[test]
fn test_game_state_init() {
    let gs = GameState::init(2, Path::new(".")).unwrap();
    assert_eq!(gs.piles[0].len(), 36);
    assert_eq!(gs.piles[1].len(), 26);
    assert_eq!(gs.piles[2].len(), 16);
    assert_eq!(gs.market[0].len(), 4);
    assert_eq!(gs.market[1].len(), 4);
    assert_eq!(gs.market[2].len(), 4);
    assert_eq!(gs.nobles.len(), 3);
}
