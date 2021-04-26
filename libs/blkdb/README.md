## blkdb: a general block-tree database

In Themelio, on-disk blockchains and "block trees" (blockchains with forks) are used in at least two different places:

- By stakers, internally, to maintain the state for neosymph's Streamlet implementation
- By all nodes to maintain the confirmed blockchain.

In both of these cases, we want an efficient, highly reliable, and spamming-resistant on-disk representation that internally manages stuff like caching and indexing. This deserves a dedicated crate for this purpose.

## Data model

The basic data model is a tree of _blocks_, each logically represented as a `(SealedState, Block, Metadata)`. In practice, of course, the `Block` would be constructed on demand, cached in a LRU cache, etc.

The following queries are supported:

- Getting all blocks at a given height
- Getting all children of a block
- Getting the parent of a block
- Getting a block by block hash
- Getting all the tips
- Attaching metadata to blocks atomically

## Implementation notes

### The raw database backend

The I/O layer is abstracted away with a trait:

```rust
pub trait DbBackend {
    type Iter: Iterator<Item = (Vec<u8>, Vec<u8>)>;

    fn insert(&self, key: &[u8], value: &[u8]);
    fn delete(&self, key: &[u8]);
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn range(&self, range: impl RangeBounds<[u8]>) -> Self::Iter;
}
```

We can implement this trait for e.g. BTreeMap and sled::Db. This lets us test blkdb extensively in a "mock" fashion.

### Indexing

We take advantage of the ordering support in the underlying database by having `be_height || hash` map to an adjacency list.
