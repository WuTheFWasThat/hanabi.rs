# Simulations of Hanabi strategies

Hanabi is a cooperative card game of incomplete information.
Despite relatively [simple rules](https://boardgamegeek.com/article/10670613#10670613),
the space of Hanabi strategies is quite interesting.
This project provides a framework for implementing Hanabi strategies in Rust.
It also explores some implementations, based on ideas from
[this paper](https://d0474d97-a-62cb3a1a-s-sites.googlegroups.com/site/rmgpgrwc/research-papers/Hanabi_final.pdf).
In particular, it contains an improved version of their "information strategy",
which achieves the best results I'm aware of for games with more than 2 players ([see below](#results)).

Please feel free to contact me about Hanabi strategies, or this framework.

Most similar projects I am aware of:
- https://github.com/rjtobin/HanSim (written for the paper mentioned above)
- https://github.com/Quuxplusone/Hanabi

## Setup

Install rust (rustc and cargo), and clone this git repo.

Then, in the repo root, run `cargo run -- -h` to see usage details.

For example, to simulate a 5 player game using the cheating strategy, for seeds 0-99:
```
cargo run -- -n 100 -s 0 -p 5 -g cheat
```

Or, if the simulation is slow, build with `--release` and use more threads:
```
time cargo run --release -- -n 10000 -o 1000 -s 0 -t 4 -p 5 -g info
```

Or, to see a transcript of the game with seed 222:
```
cargo run -- -s 222 -p 5 -g info -l debug | less
```

## Strategies

To write a strategy, you simply [implement a few traits](src/strategy.rs).

The framework is designed to take advantage of Rust's ownership system
so that you *can't cheat*, without using stuff like `Cell` or `Arc` or `Mutex`.

Generally, your strategy will be passed something of type `&BorrowedGameView`.
This game view contains many useful helper functions ([see here](src/game.rs)).
If you want to mutate a view, you'll want to do something like
`let mut self.view = OwnedGameView::clone_from(borrowed_view);`.
An OwnedGameView will have the same API as a borrowed one.

Some examples:

- [Basic dummy examples](src/strategies/examples.rs)
- [A cheating strategy](src/strategies/cheating.rs), using `Rc<RefCell<_>>`
- [The information strategy](src/strategies/information.rs)!

## Results

On seeds 0-9999, we have these average scores and win rates:

|       |   2p    |   3p    |   4p    |   5p    |
|-------|---------|---------|---------|---------|
|cheat  | 24.8600 | 24.9781 | 24.9715 | 24.9570 |
|       | 90.52 % | 98.12 % | 97.74 % | 96.57 % |
|info   | 20.9745 | 24.6041 | 24.8543 | 24.8942 |
|       | 04.40 % | 75.07 % | 89.59 % | 91.53 % |


To reproduce:
```
n=10000   # number of rounds to simulate
t=4       # number of threads
for strategy in info cheat; do
  for p in $(seq 2 5); do
    time cargo run --release -- -n $n -s 0 -t $t -p $p -g $strategy;
  done
done
```
