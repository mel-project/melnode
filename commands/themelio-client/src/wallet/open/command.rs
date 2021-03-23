use nodeprot::ValClient;
use crate::storage::ClientStorage;
use tmelcrypt::HashVal;
use blkstructs::NetID;
use colored::Colorize;

/// TODO: May need to use custom ToStr strum derives per field instead of snake_case
#[derive(Eq, PartialEq, Debug, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum OpenWalletCommand {
    Faucet(u128, String),
    // Deposit,
    // Withdraw,
    // Swap,
    SendCoins(String, u128, String),
    AddCoins(String),
    Balance,
    Help,
    Exit,
}

pub struct OpenWalletCommandHandler {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
    version: String,
    prompt: String,
    name: String,
}

impl OpenWalletCommandHandler {
    pub(crate) fn new(host: smol::net::SocketAddr, database: std::path::PathBuf, version: String, name: String) -> Self {
        let prompt_stack: Vec<String> = vec![format!("v{}", version).green().to_string()];
        let prompt = format!("[client wallet {} {}]% ", name, prompt_stack.join(" "));
        Self {
            host,
            database,
            version,
            prompt,
            name
        }
    }

    /// Parse user input into a wallet command process the command
    pub(crate) async fn handle(&self) -> anyhow::Result<OpenWalletCommand> {
        // Parse input into a command
        let input = OpenWalletCommandHandler::read_line(self.prompt.to_string())
            .await
            .unwrap();
        let cmd: OpenWalletCommand = WalletCommand::from_str(&input)?;

        // Init storage
        let storage = ClientStorage::new(sled::open(&self.database).unwrap());

        // Take snapshot
        let client = nodeprot::ValClient::new(NetID::Testnet, self.host);
        client.trust(0, HashVal::default());
        let snapshot = client.snapshot().await;

        // Process command with snapshot
        match &cmd {
            OpenWalletCommand::Faucet(_, _) => {}
            OpenWalletCommand::SendCoins(_, _, _) => {}
            OpenWalletCommand::AddCoins(_) => {}
            OpenWalletCommand::Balance => {}
            OpenWalletCommand::Help => {}
            OpenWalletCommand::Exit => {}
        };

        //flow pseudo-code - note should use ValClientSnapshot

        // get trusted staker set,
        // use height

        // - swap
        // 	- input pool, buy/sell, token name, denom, amount
        // 		- just buy & sell abc
        // 	- create tx
        // 	- presend (do we sign here?)
        // 	- send
        // 	- query
        // 	- print query results
        // 	- update storage
        // - send
        // 	- input dest, amount, denom
        // 	- create
        // 	- presend (do we sign here?) / fee calc?
        // 	- send
        // 	- query / print query results
        // 	- update storage
        // - receive
        // 	- input coin id
        // 	- query
        // 	- update storage
        // - deposit / withdraw
        // 	- input pool
        // 	- input token
        // 	- input amount (do we validate?)
        // 	- create tx
        // 	- prespend
        // 	- send
        // 	- query / print query results
        // 	- update storage
        // - faucet (enable-disable based on mainnet or not)
        // 	- input receiver address, amount, denom, amount (upper bounded?)
        // 	- create tx
        // 	- presend
        // 	- query
        // 	- update storage
        // - balance
        // 	- load storage
        // 	- print storage balance
        // - coins
        // 	- load storage
        // 	- print storage coins
        // Return processed command
        Ok(cmd)
    }

    async fn read_line(prompt: String) -> anyhow::Result<String> {
        smol::unblock(move || {
            let mut rl = rustyline::Editor::<()>::new();
            Ok(rl.readline(&prompt)?)
        }).await
    }

}
