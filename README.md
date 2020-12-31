# themelio-core: Themelio's reference implementation

[Themelio](https://themelio.org) is a work-in-progress public blockchain focused on security, performance, and long-term stability. `themelio-core` is Themelio's reference implementation in Rust.

## Organization of this repository

To simplify deployment, `themelio-core` is compiled as a monolithic binary. Much of the functionality, however, is in separate crates in the `libs/` folder:

- `autosmt`: optimized, copy-on-write [sparse Merkle trees](https://ethresear.ch/t/optimizing-sparse-merkle-trees/3751), used for storing blockchain state throughout Themelio
- `blkstructs`: structures for blockchain state elements, such as blocks and transactions.
- `melnet`: a bare-bones peer-to-peer RPC library, used by Themelio as the underlying transport for all network messages
- `melpow`: a library implementing `MelPoW`, the variation on non-interactive proof-of-work used by Themelio's instance of [Melmint](https://pdfs.semanticscholar.org/5a3e/bad5134a7b24e5557325bd5387a5ab3a7a0f.pdf)
- `melscript`: a library implementing MelScript, Themelio's on-chain scripting language.
- `symphonia`: Themelio's HotStuff-based BFT consensus protocol, implemented as a pluggable, async-first library.
- `tmelcrypt`: Themelio's cryptography library, wrapping existing trustworthy implementations in an ergonomic way.

## Use of asynchronous Rust

`themelio-core` is written in an `async/await`-first fashion. Executors are generally manually instantiated `smol` executors, and it is our intention that `themelio-core` not have large frameworks like `async-std` or `tokio` in its dependency tree.
