# reichtum

A Rust crate implementing the game logic of Reichtum.

## Testing AI via self-play

```
cargo run --release --example self_play -- --games 1000 --agents 1,1,0
```

The integer arguments to `--agents` are the "difficulty" of each agent, where
higher numbers correspond to more intelligent agents.
