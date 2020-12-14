use std::convert::TryInto;

use parking_lot::RwLock;

use crate::{
    cointype_dosc, CoinData, CoinDataHeight, CoinID, StakeDoc, State, Transaction,
    TxApplicationError, TxKind, COINTYPE_TMEL, COINTYPE_TSYM, STAKE_EPOCH,
};

// apply inputs
pub(crate) fn apply_tx_inputs(
    lself: &RwLock<State>,
    tx: &Transaction,
) -> Result<(), TxApplicationError> {
    let scripts = tx.script_as_map();
    // build a map of input coins
    let mut in_coins: im::HashMap<Vec<u8>, u64> = im::HashMap::new();
    // iterate through the inputs
    for coin_id in tx.inputs.iter() {
        let (coin_data, _) = lself.read().coins.get(coin_id);
        match coin_data {
            None => return Err(TxApplicationError::NonexistentCoin(*coin_id)),
            Some(coin_data) => {
                log::trace!(
                    "coin_data {:?} => {:?} for txid {:?}",
                    coin_id,
                    coin_data,
                    tx.hash_nosigs()
                );
                let script = scripts.get(&coin_data.coin_data.conshash).ok_or(
                    TxApplicationError::NonexistentScript(coin_data.coin_data.conshash),
                )?;
                // we skip checking the script if it's ABID and the tx type is buyout or fill
                if !(coin_data.coin_data.conshash == tmelcrypt::hash_keyed(b"ABID", b"special")
                    && (tx.kind == TxKind::AuctionBuyout || tx.kind == TxKind::AuctionFill))
                    && !script.check(tx)
                {
                    return Err(TxApplicationError::ViolatesScript(
                        coin_data.coin_data.conshash,
                    ));
                }
                // spend the coin by deleting
                lself.write().coins.delete(coin_id);
                in_coins.insert(
                    coin_data.coin_data.cointype.clone(),
                    in_coins.get(&coin_data.coin_data.cointype).unwrap_or(&0)
                        + coin_data.coin_data.value,
                );
            }
        }
    }
    // balance inputs and outputs. ignore outputs with empty cointype (they create a new token kind)
    let out_coins = tx.total_outputs();
    if tx.kind != TxKind::DoscMint && tx.kind != TxKind::Faucet {
        for (currency, value) in out_coins.iter() {
            if !currency.is_empty() && *value != *in_coins.get(currency).unwrap_or(&u64::MAX) {
                return Err(TxApplicationError::UnbalancedInOut);
            }
        }
    }
    Ok(())
}
// apply outputs
pub(crate) fn apply_tx_outputs(lself: &RwLock<State>, tx: &Transaction) {
    for (index, coin_data) in tx.outputs.iter().enumerate() {
        let height = lself.read().height;
        // if conshash is zero, this destroys the coins permanently
        if coin_data.conshash.0 != [0; 32] {
            lself.write().coins.insert(
                CoinID {
                    txhash: tx.hash_nosigs(),
                    index: index.try_into().unwrap(),
                },
                CoinDataHeight {
                    coin_data: coin_data.clone(),
                    height,
                },
            );
        }
    }
}
// apply special effects
pub(crate) fn apply_tx_special(
    lself: &RwLock<State>,
    tx: &Transaction,
) -> Result<(), TxApplicationError> {
    match tx.kind {
        TxKind::DoscMint => apply_tx_special_doscmint(lself, tx),
        TxKind::AuctionBid => apply_tx_special_auctionbid(lself, tx),
        TxKind::AuctionBuyout => apply_tx_special_auctionbuyout(lself, tx),
        TxKind::AuctionFill => {
            panic!("auction fill transaction processed in normal pipeline")
        }
        TxKind::Stake => apply_tx_special_stake(lself, tx),
        _ => panic!("tried to apply special effects of a non-special transaction"),
    }
}
// dosc minting
pub(crate) fn apply_tx_special_doscmint(
    lself: &RwLock<State>,
    tx: &Transaction,
) -> Result<(), TxApplicationError> {
    let lself = lself.read();
    // construct puzzle seed
    let chi = tmelcrypt::hash_single(
        &bincode::serialize(tx.inputs.get(0).ok_or(TxApplicationError::MalformedTx)?).unwrap(),
    );
    // compute difficulty
    let new_dosc = *tx
        .total_outputs()
        .get(&cointype_dosc(lself.height))
        .ok_or(TxApplicationError::MalformedTx)?;
    let raw_difficulty = new_dosc * lself.dosc_multiplier;
    let true_difficulty = 64 - raw_difficulty.leading_zeros() as usize;
    // check the proof
    let mp_proof = melpow::Proof::from_bytes(&tx.data).ok_or(TxApplicationError::MalformedTx)?;
    if !mp_proof.verify(&chi.0, true_difficulty) {
        Err(TxApplicationError::InvalidMelPoW)
    } else {
        Ok(())
    }
}
// auction bidding
pub(crate) fn apply_tx_special_auctionbid(
    lself: &RwLock<State>,
    tx: &Transaction,
) -> Result<(), TxApplicationError> {
    let mut lself = lself.write();
    // must be in first half of auction
    if lself.height % 20 >= 10 {
        return Err(TxApplicationError::BidWrongTime);
    }
    // data must be a 32-byte conshash
    if tx.data.len() != 32 {
        return Err(TxApplicationError::MalformedTx);
    }
    // first output stores the price bid for the mets
    let first_output = tx.outputs.get(0).ok_or(TxApplicationError::MalformedTx)?;
    if first_output.cointype != cointype_dosc(lself.height) {
        return Err(TxApplicationError::MalformedTx);
    }
    // first output must have an empty script
    if first_output.conshash != tmelcrypt::hash_keyed(b"ABID", b"special") {
        return Err(TxApplicationError::MalformedTx);
    }
    // save transaction to auction list
    lself.auction_bids.insert(tx.hash_nosigs(), tx.clone());
    Ok(())
}

// auction buyout
pub(crate) fn apply_tx_special_auctionbuyout(
    lself: &RwLock<State>,
    tx: &Transaction,
) -> Result<(), TxApplicationError> {
    let mut lself = lself.write();
    // find the one and only ABID input
    let abid_txx: Vec<Transaction> = tx
        .inputs
        .iter()
        .filter_map(|cid| lself.auction_bids.get(&cid.txhash).0)
        .collect();
    if abid_txx.len() != 1 {
        return Err(TxApplicationError::MalformedTx);
    }
    let abid_txx = &abid_txx[0];
    // validate that the first output fills the order
    let first_output: &CoinData = tx.outputs.get(0).ok_or(TxApplicationError::MalformedTx)?;
    if first_output.cointype != COINTYPE_TSYM
        || first_output.value < abid_txx.outputs[0].value
        || first_output.conshash.0.to_vec() != abid_txx.data
    {
        return Err(TxApplicationError::MalformedTx);
    }
    // remove the order from the order book
    lself.auction_bids.delete(&abid_txx.hash_nosigs());
    Ok(())
}
// stake
pub(crate) fn apply_tx_special_stake(
    lself: &RwLock<State>,
    tx: &Transaction,
) -> Result<(), TxApplicationError> {
    // first we check that the data is correct
    let stake_doc: StakeDoc =
        bincode::deserialize(&tx.data).map_err(|_| TxApplicationError::MalformedTx)?;
    let curr_epoch = lself.read().height / STAKE_EPOCH;
    // then we check that the first coin is valid
    let first_coin = tx.outputs.get(0).ok_or(TxApplicationError::MalformedTx)?;
    if first_coin.cointype != COINTYPE_TMEL.to_vec() {
        return Err(TxApplicationError::MalformedTx);
    }
    // then we check consistency
    if !(stake_doc.e_start > curr_epoch
        && stake_doc.e_post_end > stake_doc.e_start
        && stake_doc.e_start == first_coin.value)
    {
        lself.write().stakes.insert(tx.hash_nosigs(), stake_doc);
    }
    Ok(())
}
