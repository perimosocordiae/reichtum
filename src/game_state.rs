use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};

type DynError = Box<dyn std::error::Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    // 3 face down piles of cards, one per level.
    #[serde(skip)]
    piles: [Vec<Card>; 3],

    // 3 rows of face up cards, with 4 per level.
    market: [Vec<Card>; 3],
}
impl GameState {
    pub fn init(cards_file: &str) -> Result<GameState, DynError> {
        let cards = load_cards_from_csv(cards_file)?;
        let mut market = [Vec::new(), Vec::new(), Vec::new()];
        for card in cards {
            market[card.level - 1].push(card);
        }
        let mut rng = rand::thread_rng();
        market[0].shuffle(&mut rng);
        market[1].shuffle(&mut rng);
        market[2].shuffle(&mut rng);
        Ok(GameState {
            piles: [
                market[0].split_off(4),
                market[1].split_off(4),
                market[2].split_off(4),
            ],
            market,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    level: usize,
    color: Color,
    // Victory points
    vp: u8,
    // Cost to buy this card: [white, blue, green, red, black]
    cost: [u8; 5],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    White,
    Blue,
    Green,
    Red,
    Black,
}

fn load_cards_from_csv(path: &str) -> Result<Vec<Card>, DynError> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_path(path)?;
    let mut cards = Vec::new();
    for result in rdr.deserialize() {
        let record: Card = result?;
        cards.push(record);
    }
    Ok(cards)
}

#[test]
fn test_load_cards_from_csv() {
    let cards = load_cards_from_csv("cards.csv").unwrap();
    assert_eq!(cards.len(), 90);
}

#[test]
fn test_game_state_init() {
    let gs = GameState::init("cards.csv").unwrap();
    assert_eq!(gs.piles[0].len(), 36);
    assert_eq!(gs.piles[1].len(), 26);
    assert_eq!(gs.piles[2].len(), 16);
    assert_eq!(gs.market[0].len(), 4);
    assert_eq!(gs.market[1].len(), 4);
    assert_eq!(gs.market[2].len(), 4);
}
