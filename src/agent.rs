use crate::game_state::{Action, GameState};
use rand::seq::SliceRandom;

pub fn create_agent() -> Box<dyn Agent + Send> {
    Box::<RandomAgent>::default()
}

pub trait Agent {
    fn choose_action(&self, game: &GameState) -> Action;
}

#[derive(Default)]
pub struct RandomAgent;
impl Agent for RandomAgent {
    fn choose_action(&self, game: &GameState) -> Action {
        let mut rng = rand::thread_rng();
        let actions = game.valid_actions();
        if let Some(m) = actions.choose(&mut rng) {
            m.clone()
        } else {
            panic!("No moves to choose from! GameState: {:?}", game);
        }
    }
}
