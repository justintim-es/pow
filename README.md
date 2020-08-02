[![Try on playground](https://img.shields.io/badge/Playground-node_template-brightgreen?logo=Parity%20Substrate)](https://playground-staging.substrate.dev/?deploy=node-template)
# Hexsture
smart contract blockchain launch date 24/8

## hashing
Guess the right hash and earn tokens

## voting
Vote on hashes and all voters of the highest hash earn tokens

# installation ubuntu
### step 1
```
curl https://getsubstrate.io -sSf | bash -s -- --fast
```
### step 2
```
git clone https://github.com/noahsalvadordenjo/pow.git
```
## Build
### step 3
```
cargo build --release
```

## Run
### step 4
### Single Node Development Chain

Purge any existing developer chain state:

```bash
./target/release/node-template purge-chain --dev
```

Start a development chain with:

```bash
./target/release/node-template --dev
```

Detailed logs may be shown by running the node with the following environment variables set:
`RUST_LOG=debug RUST_BACKTRACE=1 cargo run -- --dev`.
