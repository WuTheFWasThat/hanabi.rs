# Simulations of Hanabi strategies

Hanabi is a cooperative card game of incomplete information.
Despite relatively [simple rules](https://boardgamegeek.com/article/10670613#10670613),
the space of Hanabi strategies is quite interesting.
This project provides a framework for implementing Hanabi strategies in Rust, and also implements extremely strong strategies.

The best strategy is based on the "information strategy" from the paper ["How to Make the Perfect Fireworks Display" by Cox et al]([How to Make the Perfect Fireworks Display](https://www.jstor.org/stable/10.4169/math.mag.88.5.323)).  See results ([below](#results)).
It held state-of-the-art results (from March 2016) until December 2019, when [researchers at Facebook](https://arxiv.org/abs/1912.02318) surpassed it by extending the idea further with explicit search.

Please feel free to contact me about Hanabi strategies, or this framework.

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

## Results (auto-generated)

To reproduce:
```
time cargo run --release -- --results-table
```

To update this file:
```
time cargo run --release -- --write-results-table
```

On the first 20000 seeds, we have these scores and win rates (average ± standard error):

|         |   2p    |   3p    |   4p    |   5p    |
|---------|------------------|------------------|------------------|------------------|
| cheat   | 24.8209 ± 0.0041 | 24.9781 ± 0.0012 | 24.9734 ± 0.0014 | 24.9618 ± 0.0017 |
|         | 88.40 ± 0.23 % | 98.14 ± 0.10 % | 97.83 ± 0.10 % | 97.03 ± 0.12 % |
| info    | 22.5217 ± 0.0125 | 24.7946 ± 0.0039 | 24.9356 ± 0.0022 | 24.9223 ± 0.0024 |
|         | 12.55 ± 0.23 % | 84.48 ± 0.26 % | 95.05 ± 0.15 % | 94.04 ± 0.17 % |

## Other work

Most similar projects I am aware of:
- https://github.com/rjtobin/HanSim (written for the paper mentioned above which introduces the information strategy)
- https://github.com/Quuxplusone/Hanabi

Some researchers are trying to solve Hanabi using machine learning techniques:
- [Initial paper](https://arxiv.org/abs/1902.00506) from DeepMind and Google Brain researchers. See [this Wall Street Journal coverage](https://www.wsj.com/articles/why-the-card-game-hanabi-is-the-next-big-hurdle-for-artificial-intelligence-11553875351)
- [This paper](https://arxiv.org/abs/1912.02318) from Facebook, code at https://github.com/facebookresearch/Hanabi_SPARTA which includes their machine-learned agent
