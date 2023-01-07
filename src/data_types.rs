use serde::{Deserialize, Serialize};

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
    pub level: usize,
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
