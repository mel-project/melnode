use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
};

use super::{friendly_denom, MicroUnit, RenderTimeTracer};
use crate::{notfound, to_badgateway, to_badreq};
use anyhow::Context;
use askama::Template;
use themelio_nodeprot::ValClient;
use themelio_stf::{melvm::Address, CoinData, CoinDataHeight, CoinID, NetID, Transaction, TxHash};

#[derive(Template)]
#[template(path = "transaction.html")]
struct TransactionTemplate {
    testnet: bool,
    txhash: TxHash,
    txhash_abbr: String,
    height: u64,
    transaction: Transaction,
    inputs_with_cdh: Vec<(usize, CoinID, CoinDataHeight, MicroUnit)>,
    outputs: Vec<(usize, CoinData, MicroUnit)>,
    fee: MicroUnit,
    base_fee: MicroUnit,
    tips: MicroUnit,
    net_loss: BTreeMap<String, Vec<MicroUnit>>,
    net_gain: BTreeMap<String, Vec<MicroUnit>>,
    gross_gain: Vec<MicroUnit>,
}

#[tracing::instrument(skip(req))]
#[allow(clippy::comparison_chain)]
pub async fn get_txpage(req: tide::Request<ValClient>) -> tide::Result<tide::Body> {
    let _render = RenderTimeTracer::new("txpage");

    let height: u64 = req.param("height").unwrap().parse().map_err(to_badreq)?;
    let txhash: TxHash = TxHash(req.param("txhash").unwrap().parse().map_err(to_badreq)?);
    let snap = req
        .state()
        .snapshot()
        .await
        .map_err(to_badgateway)?
        .get_older(height)
        .await
        .map_err(to_badgateway)?;
    let transaction = snap
        .get_transaction(txhash)
        .await
        .map_err(to_badgateway)?
        .ok_or_else(notfound)?;

    // now that we have the transaction, we can construct the info.
    let denoms: BTreeSet<_> = transaction
        .outputs
        .iter()
        .map(|v| -> themelio_stf::Denom { v.denom })
        .collect();
    let mut net_loss: BTreeMap<String, Vec<MicroUnit>> = BTreeMap::new();
    let mut net_gain: BTreeMap<String, Vec<MicroUnit>> = BTreeMap::new();
    for denom in denoms {
        let mut balance: BTreeMap<Address, i128> = BTreeMap::new();
        // we add to the balance
        for output in transaction.outputs.iter() {
            if output.denom == denom {
                let new_balance = balance
                    .get(&output.covhash)
                    .cloned()
                    .unwrap_or_default()
                    .checked_add(output.value.try_into()?)
                    .context("cannot add")?;
                balance.insert(output.covhash, new_balance);
            }
        }
        // we subtract from the balance
        for input in transaction.inputs.iter().copied() {
            let cdh = snap
                .get_coin_spent_here(input)
                .await?
                .context("no CDH found for one of the inputs")?;
            if cdh.coin_data.denom == denom {
                let new_balance = balance
                    .get(&cdh.coin_data.covhash)
                    .cloned()
                    .unwrap_or_default()
                    .checked_sub(cdh.coin_data.value.try_into()?)
                    .context("cannot add")?;
                balance.insert(cdh.coin_data.covhash, new_balance);
            }
        }
        // we update net loss/gain
        for (addr, balance) in balance {
            if balance < 0 {
                net_loss
                    .entry(addr.0.to_addr())
                    .or_default()
                    .push(MicroUnit((-balance) as u128, friendly_denom(denom)));
            } else if balance > 0 {
                net_gain
                    .entry(addr.0.to_addr())
                    .or_default()
                    .push(MicroUnit(balance as u128, friendly_denom(denom)));
            }
        }
    }

    let fee = transaction.fee;
    let fee_mult = snap
        .get_older(height - 1)
        .await
        .map_err(to_badgateway)?
        .current_header()
        .fee_multiplier;
    let base_fee = transaction.base_fee(fee_mult, 0);
    let tips = fee.saturating_sub(base_fee);

    let mut inputs_with_cdh = vec![];
    // we subtract from the balance
    for (index, input) in transaction.inputs.iter().copied().enumerate() {
        let cdh = snap
            .get_coin_spent_here(input)
            .await?
            .context("no CDH found for one of the inputs")?;
        inputs_with_cdh.push((
            index,
            input,
            cdh.clone(),
            MicroUnit(cdh.coin_data.value, friendly_denom(cdh.coin_data.denom)),
        ));
    }

    let mut body: tide::Body = TransactionTemplate {
        testnet: req.state().netid() == NetID::Testnet,
        txhash,
        txhash_abbr: hex::encode(&txhash.0[..5]),
        height,
        transaction: transaction.clone(),
        net_loss,
        inputs_with_cdh,
        net_gain,
        outputs: transaction
            .outputs
            .iter()
            .enumerate()
            .map(|(i, cd)| (i, cd.clone(), MicroUnit(cd.value, friendly_denom(cd.denom))))
            .collect(),
        fee: MicroUnit(fee, "MEL".into()),
        base_fee: MicroUnit(base_fee, "MEL".into()),
        tips: MicroUnit(tips, "MEL".into()),
        gross_gain: transaction
            .total_outputs()
            .iter()
            .map(|(denom, val)| MicroUnit(*val, friendly_denom(*denom)))
            .collect(),
    }
    .render()
    .unwrap()
    .into();
    body.set_mime("text/html");
    Ok(body)
}
