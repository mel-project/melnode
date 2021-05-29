# themelio-core: Themelio's reference implementation

[Themelio](https://themelio.org) is a work-in-progress public blockchain focused on security, performance, and long-term stability. `themelio-core` is Themelio's reference implementation in Rust.

## Organization of this repository

This repository contains the core node implementation of Themelio, as well as some supporting libraries

`commands`: Core command-line programs

- **`themelio-node`: Reference Themelio implementation**
- `themelio-bxms`: Block-explorer microservice (will be moved outside this repo)
- `themelio-crypttool`: Tool for generating keys, hashing, and other cryptographic tools

`libs`: supporting libraries

- `blkdb`: a "block database" library for ergonomically and correctly working with trees of blocks
- `melnet`: a bare-bones peer-to-peer RPC library, used by Themelio as the underlying transport for all network messages
- `nodeprot`: a higher-level library that implements the node RPC protocol on top of melnet
- `novasymph`: an instantiation of the Streamlet-based Symphonia consensus protocol for use in Themelio
