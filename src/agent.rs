use crate::data_types::Action;
use crate::game_state::GameState;
use rand::seq::SliceRandom;

pub fn create_agent(difficulty: usize) -> Box<dyn Agent + Send> {
    match difficulty {
        0 => Box::<RandomAgent>::default(),
        _ => Box::<GreedyAgent>::default(),
    }
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

#[derive(Default)]
pub struct GreedyAgent;
impl Agent for GreedyAgent {
    fn choose_action(&self, game: &GameState) -> Action {
        let actions = game.valid_actions();
        let scored_actions = actions
            .iter()
            .map(|a| (a, score_action(game, a)))
            .collect::<Vec<_>>();
        let best_score = scored_actions
            .iter()
            .map(|(_, s)| s)
            .max()
            .expect("No moves to choose from!");
        let best_actions: Vec<&Action> = scored_actions
            .iter()
            .filter(|(_, s)| s == best_score)
            .map(|(a, _)| *a)
            .collect();
        let mut rng = rand::thread_rng();
        let best = best_actions.choose(&mut rng).unwrap();
        (*best).clone()
    }
}

fn score_action(game: &GameState, action: &Action) -> i32 {
    match action {
        Action::BuyCard(loc) => {
            let card = game.peek_card(loc).unwrap();
            card.vp as i32
        }
        Action::ReserveCard(_loc) => -1,
        Action::TakeDifferentColorTokens(_colors) => -1,
        Action::TakeSameColorTokens(_color) => -1,
    }
}
