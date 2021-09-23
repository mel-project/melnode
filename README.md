# themelio-core: Themelio's reference implementation

[Themelio](https://themelio.org) is a work-in-progress public blockchain focused on security, performance, and long-term stability. `themelio-core` is Themelio's reference implementation in Rust.

## Organization of this repository

This repository contains the core node implementation of Themelio, as well as some supporting libraries

`commands`: Core command-line programs

- **`themelio-node`: Themelio' reference full node implementation**
- `themelio-crypttool`: Tool for generating keys, hashing, and other cryptographic tools

`libs`: supporting libraries

- `blkdb`: a "block database" library for ergonomically and correctly working with trees of blocks
- `novasymph`: an instantiation of the Streamlet-based Symphonia consensus protocol for use in Themelio

## Usage

Have a look in [how-to-use](/how-to-use).

## Metrics

Read [here](Metrics.md).
