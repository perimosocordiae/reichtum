use reichtum::agent::create_agent;
use reichtum::game_state::GameState;

fn main() {
    let num_games = 1000;
    let num_players = 2;
    let players = (0..num_players).map(create_agent).collect::<Vec<_>>();
    let mut scores = (0..num_players)
        .map(|_| Vec::<i32>::new())
        .collect::<Vec<_>>();
    for i in 0..num_games {
        if (i + 1) % 100 == 0 {
            println!("Game {}/{}", i + 1, num_games);
        }
        let mut gs = GameState::init(num_players).expect("Failed to initialize game state");
        for _turn in 1..=1000 {
            let action = players[gs.curr_player_idx].choose_action(&gs);
            match gs.take_turn(&action) {
                Ok(true) => break,
                Ok(false) => (),
                Err(e) => {
                    println!(
                        "{:?} for agent {} action: {:?}",
                        e, gs.curr_player_idx, action
                    );
                    println!("{:?}", gs);
                    panic!("Agent logic error")
                }
            };
        }
        gs.players.iter().enumerate().for_each(|(i, p)| {
            scores[i].push(p.vp() as i32);
        });
    }
    for (i, i_scores) in scores.iter().enumerate() {
        let total = i_scores.iter().sum::<i32>() as f32;
        let average = total / num_games as f32;
        println!("Agent {}: {:?}", i, average);
    }
    // TODO:
    //  - Show score box plots for each player
    //  - Compute rankings for each game, then summarize those
    //  - Run a wilcoxon signed-rank test to check for significance
    //  - Compute running Elo ratings for each player and plot them
}
