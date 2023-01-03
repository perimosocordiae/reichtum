use reichtum::agent::create_agent;
use reichtum::game_state::GameState;

fn main() {
    let num_players = 2;
    let players = (0..num_players).map(|_| create_agent()).collect::<Vec<_>>();
    let mut gs = GameState::init(num_players).expect("Failed to initialize game state");
    for turn in 1.. {
        let action = players[gs.curr_player_idx].choose_action(&gs);
        println!("Turn {}: P{} => {:?}", turn, gs.curr_player_idx, action);
        if gs.take_turn(&action).expect("Failed to take turn") {
            break;
        }
    }
    gs.players.iter().enumerate().for_each(|(i, p)| {
        println!("Player {}: {}", i, p.vp());
    });
}
