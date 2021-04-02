use crate::storage::WalletStorage;
use crate::wallet::common::read_line;
use crate::wallet::data::WalletData;
use blkstructs::NetID;
use colored::Colorize;
use nodeprot::ValClientSnapshot;
use serde::{Deserialize, Serialize};
use serde_scan::ScanError;
use std::convert::TryFrom;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpenWalletCommand {
    Faucet(String, String),
    Deposit,
    Withdraw,
    Swap,
    SendCoins(String, String, String),
    AddCoins(String),
    Balance,
    Help,
    Exit,
}

impl TryFrom<String> for OpenWalletCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<OpenWalletCommand, _> = serde_scan::from_str(&value);
        cmd
    }
}

pub struct OpenWalletCommandHandler {
    host: smol::net::SocketAddr,
    version: String,
    name: String,
    secret: String,
    wallet: WalletData,
    prompt: String,
}

impl OpenWalletCommandHandler {
    pub(crate) fn new(
        host: smol::net::SocketAddr,
        version: &str,
        name: &str,
        secret: &str,
        wallet: WalletData,
    ) -> Self {
        let version = version.to_string();
        let name = name.to_string();
        let secret = secret.to_string();
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
            format!("wallet ({}) ➜ ", name).cyan().italic().to_string(),
        ];
        let prompt = format!("{}", prompt_stack.join(" "));
        Self {
            host,
            version,
            name,
            secret,
            wallet,
            prompt,
        }
    }

    /// Parse user input into a wallet command process the command
    pub(crate) async fn handle(
        &self,
        storage: &WalletStorage,
    ) -> anyhow::Result<OpenWalletCommand> {
        // Convert valid user input into a command is
        let input = read_line(self.prompt.to_string()).await;
        if input.is_err() {
            eprintln!(
                ">> {}: unable to retreive or process input from user",
                "ERROR".red().bold(),
            );
            return Ok(OpenWalletCommand::Exit);
        }
        let cmd = OpenWalletCommand::try_from(input.unwrap());
        if cmd.is_err() {
            anyhow::bail!("Unable to parse command");
        }
        let cmd = cmd.unwrap();

        // Take snapshot of latest state
        // TODO: this is trusting node, gotta figure out a simple / less trusted mode
        // where it starts from geneses, but stores / caches state
        let client = nodeprot::ValClient::new(NetID::Testnet, self.host);
        let snapshot = client.snapshot_latest().await.unwrap(); // fix error handling

        // // Process command with snapshot
        // match &cmd {
        //     OpenWalletCommand::Faucet(amount, denom) => { self.faucet(&snapshot, *amount, denom).await?; }
        //     OpenWalletCommand::SendCoins(dest, amount, denom) => { self.send_coins(&snapshot, dest, amount, denom).await?; }
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
    async fn add_coins(&self, storage: &WalletStorage, coin_id: &String) -> anyhow::Result<()> {
        // - receive
        // 	- input coin id
        // 	- query
        // 	- update storage
        anyhow::bail!("Not Implemented")
    }
    async fn balance(&self, storage: &WalletStorage) -> anyhow::Result<()> {
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
