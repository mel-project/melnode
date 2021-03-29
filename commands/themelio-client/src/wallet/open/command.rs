use crate::storage::ClientStorage;
use crate::wallet::common::read_line;
use blkstructs::NetID;
use colored::Colorize;
use nodeprot::{ValClientSnapshot};
use std::str::FromStr;
use strum_macros::EnumString;
use crate::wallet::data::WalletData;

#[derive(Eq, PartialEq, Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum OpenWalletCommand {
    Faucet(u128, String),
    Deposit,
    Withdraw,
    Swap,
    SendCoins(String, u128, String),
    AddCoins(String),
    Balance,
    Help,
    Exit,
}

pub struct OpenWalletCommandHandler {
    host: smol::net::SocketAddr,
    version: String,
    name: String,
    wallet: WalletData,
    prompt: String,
}

impl OpenWalletCommandHandler {
    pub(crate) fn new(
        host: smol::net::SocketAddr,
        version: String,
        name: String,
        wallet: WalletData,
    ) -> Self {
        let prompt_stack: Vec<String> = vec![format!("v{}", version).green().to_string()];
        let prompt = format!(
            "[client wallet {} {}]% ",
            "replace_with_wallet_name",
            prompt_stack.join(" ")
        );
        Self {
            host,
            version,
            name,
            wallet,
            prompt,
        }
    }

    /// Parse user input into a wallet command process the command
    pub(crate) async fn handle(&self) -> anyhow::Result<OpenWalletCommand> {
        // Parse input into a command
        let input = read_line(self.prompt.to_string()).await;
        if input.is_err() {
            return Ok(OpenWalletCommand::Exit);
        }
        let cmd: OpenWalletCommand = OpenWalletCommand::from_str(&input.unwrap())?;

        // Init storage from wallet name
        // let storage = ClientStorage::new(sled::open(&self.database).unwrap());

        // Take snapshot of latest state
        let client = nodeprot::ValClient::new(NetID::Testnet, self.host);
        let snapshot = client.snapshot_latest().await.unwrap(); // fix error handling

        // Process command with snapshot
        // match &cmd {
        //     OpenWalletCommand::Faucet(amount, denom) => { self.faucet(&snapshot, *amount, denom).await?; }
        //     OpenWalletCommand::SendCoins(dest, amount, denom) => { self.send_coins(&snapshot, dest, *amount, denom).await?; }
        //     OpenWalletCommand::AddCoins(coin_id) => { self.add_coins(&storage, coin_id).await?; }
        //     OpenWalletCommand::Balance => { self.balance(&storage).await?; }
        //     OpenWalletCommand::Help => { self.help().await?; }
        //     OpenWalletCommand::Exit => {}
        //     OpenWalletCommand::Deposit => {}
        //     OpenWalletCommand::Withdraw => {}
        //     OpenWalletCommand::Swap => {}
        // };

        // Return processed command with args
        // Ok(cmd)
        Ok(OpenWalletCommand::Exit)
    }

    async fn faucet(
        &self,
        snapshot: &ValClientSnapshot,
        amount: u128,
        denom: &String,
    ) -> anyhow::Result<()> {
        // - faucet (enable-disable based on mainnet or not)
        // 	- input receiver address, amount, denom, amount (upper bounded?)
        // 	- create tx
        // 	- presend
        // 	- query
        // 	- update storage
        anyhow::bail!("Not Implemented")
    }

    async fn send_coins(
        &self,
        snapshot: &ValClientSnapshot,
        dest: &String,
        amount: u128,
        denom: &String,
    ) -> anyhow::Result<()> {
        // - send
        // 	- input dest, amount, denom
        // 	- create
        // 	- presend (do we sign here?) / fee calc?
        // 	- send
        // 	- query / print query results
        // 	- update storage
        anyhow::bail!("Not Implemented")
    }
    async fn add_coins(&self, storage: &ClientStorage, coin_id: &String) -> anyhow::Result<()> {
        // - receive
        // 	- input coin id
        // 	- query
        // 	- update storage
        anyhow::bail!("Not Implemented")
    }
    async fn balance(&self, storage: &ClientStorage) -> anyhow::Result<()> {
        // - balance
        // 	- load storage
        // 	- print storage balance
        // - coins
        // 	- load storage
        // 	- print storage coins
        anyhow::bail!("Not Implemented")
    }
    async fn help(&self) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn deposit(&self, snapshot: &ValClientSnapshot) -> anyhow::Result<()> {
        // - deposit
        // 	- input pool
        // 	- input token
        // 	- input amount (do we validate?)
        // 	- create tx
        // 	- prespend
        // 	- send
        // 	- query / print query results
        // 	- update storage
        anyhow::bail!("Not Implemented")
    }
    async fn withdraw(&self, snapshot: &ValClientSnapshot) -> anyhow::Result<()> {
        // - withdraw
        // 	- input pool
        // 	- input token
        // 	- input amount (do we validate?)
        // 	- create tx
        // 	- prespend
        // 	- send
        // 	- query / print query results
        // 	- update storage
        anyhow::bail!("Not Implemented")
    }
    async fn swap(&self, snapshot: &ValClientSnapshot) -> anyhow::Result<()> {
        // - swap
        // 	- input pool, buy/sell, token name, denom, amount
        // 		- just buy & sell abc
        // 	- create tx
        // 	- presend (do we sign here?)
        // 	- send
        // 	- query
        // 	- print query results
        // 	- update storage
        anyhow::bail!("Not Implemented")
    }
}
