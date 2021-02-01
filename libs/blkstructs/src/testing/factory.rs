use crate::{TxKind, CoinID, Transaction, CoinData, DENOM_TMEL};
use tmelcrypt::{Ed25519PK, Ed25519SK};
use crate::melscript::Script;

pub fn tx_factory(
    kind: TxKind,
    sender_keypair: (Ed25519PK, Ed25519SK),
    dest_pk: Ed25519PK,
    coin_id: CoinID,
    script: Script,
    value: u64,
    fee: u64
) -> Transaction {
    let tx = Transaction {
        kind,
        inputs: vec![coin_id],
        outputs: vec![CoinData {
            covhash: Script::std_ed25519_pk(dest_pk).hash(),
            value: value - fee,
            denom: DENOM_TMEL.to_owned(),
        }],
        fee,
        scripts: vec![script],
        data: vec![],
        sigs: vec![]
    };

    // Sign transaction and return tx
    tx.sign_ed25519(sender_keypair.1)
}