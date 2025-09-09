use clap::Parser;
use indicatif::ProgressIterator;
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
    let (names, scores) = run_games(args.games, &args.agents);
    let mut writer = csv::Writer::from_writer(std::io::stdout());
    writer.write_record(&names).unwrap();
    for row in &scores {
        writer.serialize(row).unwrap();
    }
}

fn run_games(num_games: usize, agents: &[usize]) -> (Vec<String>, Vec<Vec<i32>>) {
    let num_players = agents.len();
    let players = agents
        .iter()
        .map(|lvl| create_agent(*lvl))
        .collect::<Vec<_>>();
    let names = agents
        .iter()
        .enumerate()
        .map(|(i, lvl)| format!("{}(d={})", (i as u8 + b'A') as char, lvl))
        .collect::<Vec<_>>();
    let mut scores = Vec::new();
    for _ in (0..num_games).progress() {
        let mut gs = GameState::init(num_players).expect("Failed to initialize game state");
        for _turn in 1..=1000 {
            let action = players[gs.curr_player_idx].choose_action(&gs);
            match gs.take_turn(&action) {
                Ok(true) => break,
                Ok(false) => (),
                Err(e) => {
                    println!(
                        "{:?} for agent {} action: {:?}",
                        e, &names[gs.curr_player_idx], action
                    );
                    println!("{:?}", gs);
                    panic!("Agent logic error")
                }
            };
        }
        scores.push(gs.players.into_iter().map(|p| p.vp() as i32).collect());
    }
    (names, scores)
}
