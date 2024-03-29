# How to run this collator

First start two validators that will run for the relay chain:

```sh
cargo run --release -- -d alice --chain cardamom-local --validator --alice --port 50551
cargo run --release -- -d bob --chain cardamom-local --validator --bob --port 50552
```

Next start the collator that will collate for the adder parachain:

```sh
cargo run --release -p test-parachain-adder-collator -- --tmp --chain cardamom-local --port 50553
```

The last step is to register the parachain using selendra-js. The parachain id is
100. The genesis state and the validation code are printed at startup by the collator.

To do this automatically, run `scripts/adder-collator.sh`.
