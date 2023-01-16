use clap::Parser;
use indicatif::ProgressIterator;
use polars::prelude::*;
use reichtum::agent::create_agent;
use reichtum::game_state::GameState;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value_t = 1000)]
    games: usize,
    #[clap(short, long, value_delimiter = ',', default_value = "0,1")]
    agents: Vec<usize>,
    #[clap(short, long, default_value_t = false)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();
    let mut scores = run_games(args.games, &args.agents);
    if args.verbose {
        CsvWriter::new(&mut std::io::stdout())
            .has_header(true)
            .finish(&mut scores)
            .unwrap();
        return;
    }
    println!("Scores: {}", &scores.describe(None));

    let agent_names = scores.get_column_names();
    let rank_opts = RankOptions {
        method: RankMethod::Ordinal,
        descending: true,
    };
    let each_column = (0..args.agents.len())
        .map(|i| col("rank").arr().get(lit(i as i64)).alias(agent_names[i]))
        .collect::<Vec<_>>();
    let rankings = scores
        .lazy()
        .with_column(concat_lst([all()]).alias("all_scores"))
        .select([col("all_scores")
            .arr()
            .eval(col("").rank(rank_opts), true)
            .alias("rank")])
        .select(each_column)
        .collect()
        .unwrap();
    println!("Rankings (1=winner): {}", &rankings.describe(None));

    // TODO:
    //  - Show score box plots for each player
    //  - Run a wilcoxon signed-rank test to check for significance
    //  - Compute running Elo ratings for each player and plot them
}

fn run_games(num_games: usize, agents: &[usize]) -> DataFrame {
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
    let mut scores = (0..num_players)
        .map(|_| Vec::<i32>::new())
        .collect::<Vec<_>>();
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
        gs.players.iter().enumerate().for_each(|(i, p)| {
            scores[i].push(p.vp() as i32);
        });
    }
    let columns = names
        .iter()
        .enumerate()
        .map(|(i, name)| Series::new(name, &scores[i]))
        .collect::<Vec<_>>();
    DataFrame::new(columns).unwrap()
}
