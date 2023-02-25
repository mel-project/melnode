The code is organized using the Actor pattern, using `Actor` defined in `common.rs`.

There are the following actors:

- `Auditor` handles network requests on the replica network. It periodicially syncs state with neighbors. It handles internal requests to forward transactions and consume new blocks.

- `Stakeholder` holds keys and participates in symphonia consensus. It uses a timer to wait 30 seconds, grab the current state from `Auditor` to form a block proposal, and run the consensus algorithm. When a block is decided, it is pushed to `Storage` with its consensus proof. `Stakeholder` doesn't run at all if the node is not a stakeholder.

As well as `Storage`, which is not an actor and does the following functions:

- Save every new block with its consensus proof, pruning way-too-old blocks

- Keep track of the current state

- Sync to disk atomically periodically

- Load from disk on creation

### RPC verbs

- `newtx` takes in a Transaction and returns a bool

- `gettx` takes in a txhash and returns a transaction

- `newblk` takes in a NewBlockReq and returns a NewBlockResp. Req includes a consensus proof, a header, and a list of TXIDs. Resp is either a confirmation, or a list of transactions that are missing. Req should be repeated with a list of missing transactions attached.
