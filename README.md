# themelio-node: Themelio's reference implementation

[![](https://img.shields.io/crates/v/themelio-node)](https://crates.io/crates/themelio-node)
![](https://img.shields.io/crates/l/themelio-node)

[Themelio](https://themelio.org) is a new public blockchain focused on security, performance, and long-term stability. `themelio-node` is Themelio's reference implementation in Rust.

## Overview

`themelio-node` is a highly concurrent program where different tasks are done by separate _actors_, which are "active" structs that own background async tasks or threads. They concurrently run and communicate both with other actors and with "plain data" types like `Mempool`. They are represented as green boxes in the following diagram illustrating the _data flows_ of the whole program:

![](diagram.png)

There are the following primary types in themelio-node:

- `NodeProtocol` is the **core node actor**. It implements the core auditor/full node logic: gossiping with other through the `melnet`-based auditor P2P network (via the `themelio-nodeprot` crate) to synchronize the latest blockchain state.
  - Pushes new blocks to `Storage`
  - Pushes new transactions to `Mempool`; they are only gossipped further if `Mempool` accepts them
  - Pulls data from `Storage` to gossip to other nodes
  - Pulls data from `BlockIndexer` to answer queries about coin lists (which coins do this address "own")
- `BlockIndexer` indexes "nonessential" information about blocks. It continually pulls blocks out of `Storage`, and indexes them, keeping track of information such as which coins do which address own.
- `Storage` encapsulates all persistent storage in the system. It is not an actor, so it does not initiate any data flows.
  - Stores data using `meshanina` (for sparse Merkle tree nodes) and `boringdb` (for blocks and other metadata)
  - `Mempool` is a non-persistent field that keeps track of _the most likely next block_. This is based on the existing blockchain state plus unconfirmed transactions seen in the network.
- `StakerProtocol` is the **core staker actor**, which is only started in staker mode. It runs the Streamlet consensus protocol (implemented in the `novasymph` crate) over a separate melnet P2P.
  - Pushes freshly finalized blocks, with their consensus proofs (a quorum of signatures), into `Storage` (where `NodeProtocol` will pick them up and gossip them)
  - When proposing a block, pulls a candidate from `Mempool`

## Usage

Have a look in [how-to-use](/how-to-use).

## Metrics

Themelio Labs runs a worldwide network of Themelio full nodes --- `themelio-node` can also be compiled to report metrics for this network.

Read [here](Metrics.md).
