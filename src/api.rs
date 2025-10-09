use blau_api::{GameAPI, PlayerInfo, Result};
use serde::{Deserialize, Serialize};

use crate::{
    agent::{Agent, create_agent},
    data_types::Action,
    game_state::GameState,
};

/// Final data to store for viewing completed games.
#[derive(Serialize, Deserialize)]
struct FinalState {
    game: GameState,
    scores: Vec<i32>,
}

/// Message sent to human players after each turn.
#[derive(Debug, Serialize)]
struct TakeTurnMessage<'a> {
    game_data: &'a GameState,
    is_over: bool,
    winner_id: Option<&'a str>,
}

pub struct ReichtumAPI {
    // Current game state
    state: GameState,
    // Player IDs in the same order as agents
    player_ids: Vec<String>,
    // None if human player
    agents: Vec<Option<Box<dyn Agent + Send>>>,
    // Indicates if the game is over
    game_over: bool,
}

impl ReichtumAPI {
    fn view(&self, _player_idx: usize) -> Result<String> {
        Ok(serde_json::to_string(&self.state)?)
    }
    fn winner_id(&self) -> Option<&str> {
        if !self.state.is_finished() {
            return None;
        }
        let max_vp = self.state.players.iter().map(|p| p.vp()).max().unwrap();
        let max_indices = self
            .state
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.vp() == max_vp)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        // TODO: Check for ties, break by number of owned cards.
        Some(&self.player_ids[max_indices[0]])
    }
    fn do_action<F: FnMut(&str, &str)>(&mut self, action: &Action, mut notice_cb: F) -> Result<()> {
        self.game_over = self.state.take_turn(action)?;
        // Notify all human players of the action.
        let msg = TakeTurnMessage {
            game_data: &self.state,
            is_over: self.game_over,
            winner_id: self.winner_id(),
        };
        let msg = serde_json::to_string(&msg)?;
        for idx in self.human_player_idxs() {
            notice_cb(self.player_ids[idx].as_str(), &msg);
        }
        Ok(())
    }
    fn human_player_idxs(&self) -> impl Iterator<Item = usize> + '_ {
        self.agents.iter().enumerate().filter_map(
            |(idx, agent)| {
                if agent.is_none() { Some(idx) } else { None }
            },
        )
    }
    fn process_agents<F: FnMut(&str, &str)>(&mut self, mut notice_cb: F) -> Result<()> {
        while !self.game_over
            && let Some(ai) = &self.agents[self.state.curr_player_idx]
        {
            let action = ai.choose_action(&self.state);
            self.do_action(&action, &mut notice_cb)?;
        }
        Ok(())
    }
}
impl GameAPI for ReichtumAPI {
    fn init(players: &[PlayerInfo], _params: Option<&str>) -> Result<Self> {
        let state = GameState::init(players.len())?;
        let player_ids = players.iter().map(|p| p.id.clone()).collect();
        let agents = players
            .iter()
            .map(|p| p.level.map(|lvl| create_agent(1 + lvl as usize)))
            .collect();
        Ok(Self {
            state,
            player_ids,
            agents,
            game_over: false,
        })
    }

    fn restore(player_ids: &[PlayerInfo], final_state: &str) -> Result<Self> {
        let fs: FinalState = serde_json::from_str(final_state)?;
        Ok(Self {
            state: fs.game,
            player_ids: player_ids.iter().map(|p| p.id.clone()).collect(),
            agents: Vec::new(), // No agents when restoring
            game_over: true,
        })
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn final_state(&self) -> Result<String> {
        if !self.game_over {
            return Err("Game is not finished".into());
        }
        let fs = FinalState {
            game: self.state.clone(),
            scores: self.player_scores(),
        };
        Ok(serde_json::to_string(&fs)?)
    }

    fn player_view(&self, player_id: &str) -> Result<String> {
        let player_idx = self
            .player_ids
            .iter()
            .position(|id| id == player_id)
            .ok_or("Unknown player ID")?;
        self.view(player_idx)
    }

    fn start<F: FnMut(&str, &str)>(&mut self, game_id: i64, mut notice_cb: F) -> Result<()> {
        let msg = format!(r#"{{"action": "start", "game_id": {game_id}}}"#);
        for idx in self.human_player_idxs() {
            notice_cb(self.player_ids[idx].as_str(), &msg);
        }
        // Advance to wait for the next player action.
        self.process_agents(notice_cb)?;
        Ok(())
    }

    fn process_action<F: FnMut(&str, &str)>(
        &mut self,
        action: &str,
        mut notice_cb: F,
    ) -> Result<()> {
        if self.game_over {
            return Err("Game is over".into());
        }
        let action: Action = serde_json::from_str(action)?;
        self.do_action(&action, &mut notice_cb)?;
        // Advance to wait for the next player action.
        self.process_agents(&mut notice_cb)?;
        Ok(())
    }

    fn current_player_id(&self) -> &str {
        self.player_ids[self.state.curr_player_idx].as_str()
    }

    fn player_scores(&self) -> Vec<i32> {
        self.state.players.iter().map(|p| p.vp() as i32).collect()
    }
}

#[test]
fn exercise_api() {
    let players = vec![
        PlayerInfo::human("foo".into()),
        PlayerInfo::ai("bot".into(), 1),
    ];
    let mut game: ReichtumAPI = GameAPI::init(&players, None).unwrap();
    let mut num_notices = 0;
    game.start(1234, |id, msg| {
        assert_eq!(id, "foo");
        if num_notices == 0 {
            assert_eq!(msg, "{\"action\": \"start\", \"game_id\": 1234}");
        } else {
            assert!(msg.starts_with("{"));
        }
        num_notices += 1;
    })
    .unwrap();

    let view_json = game.player_view("foo").unwrap();
    assert!(view_json.starts_with("{"));

    num_notices = 0;
    game.process_action(r#"{"ReserveCard": {"Pile": 1}}"#, |id, msg| {
        assert_eq!(id, "foo");
        assert!(msg.starts_with("{"));
        num_notices += 1;
    })
    .unwrap();
    // foo's turn and bot's turn should both generate notices.
    assert_eq!(num_notices, 2);
}
