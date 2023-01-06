use clap::Parser;
use polars::prelude::*;
use reichtum::agent::create_agent;
use reichtum::game_state::GameState;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value_t = 1000)]
    games: usize,
    #[clap(short, long, value_delimiter = ',', default_value = "0,1")]
    agents: Vec<usize>,
}

fn main() {
    let args = Args::parse();
    let num_players = args.agents.len();
    let players = args
        .agents
        .clone()
        .into_iter()
        .map(create_agent)
        .collect::<Vec<_>>();
    let agent_names = args
        .agents
        .iter()
        .enumerate()
        .map(|(i, lvl)| format!("{}(d={})", (i as u8 + b'A') as char, lvl))
        .collect::<Vec<_>>();
    let mut scores = (0..num_players)
        .map(|_| Vec::<i32>::new())
        .collect::<Vec<_>>();
    for i in 0..args.games {
        if (i + 1) % 100 == 0 {
            println!("Game {}/{}", i + 1, args.games);
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
                        e, &agent_names[gs.curr_player_idx], action
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
    let columns = agent_names
        .iter()
        .enumerate()
        .map(|(i, name)| Series::new(name, &scores[i]))
        .collect::<Vec<_>>();
    let df = DataFrame::new(columns).unwrap();
    println!("{}", &df.describe(None));

    // TODO:
    //  - Show score box plots for each player
    //  - Compute rankings for each game, then summarize those
    //  - Run a wilcoxon signed-rank test to check for significance
    //  - Compute running Elo ratings for each player and plot them
}
