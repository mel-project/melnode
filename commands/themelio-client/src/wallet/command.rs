use std::path::PathBuf;
use std::str::FromStr;

use strum_macros::EnumString;

use crate::wallet::common::read_line;
use crate::wallet::open::command::{OpenWalletCommand, OpenWalletCommandHandler};
use colored::Colorize;

#[derive(Eq, PartialEq, Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum WalletCommand {
    Create(String),
    Import(PathBuf),
    Export(PathBuf),
    Show,
    Open(String),
    Help,
    Exit,
}

pub struct WalletCommandHandler {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
    version: String,
    prompt: String,
}

impl WalletCommandHandler {
    pub(crate) fn new(
        host: smol::net::SocketAddr,
        database: std::path::PathBuf,
        version: String,
    ) -> Self {
        let prompt_stack: Vec<String> = vec![format!("v{}", version).green().to_string()];
        let prompt = format!("[client wallet {}]% ", prompt_stack.join(" "));
        Self {
            host,
            database,
            version,
            prompt,
        }
    }

    /// Parse user input into a wallet command process the command
    pub(crate) async fn handle(&self) -> anyhow::Result<WalletCommand> {
        // Parse input into a command
        let input = read_line(self.prompt.to_string()).await;
        if input.is_err() {
            return Ok(WalletCommand::Exit);
        }
        let cmd: WalletCommand = WalletCommand::from_str(&input.unwrap())?;

        // Process command
        // let client = nodeprot::ValClient::new(NetID::Testnet, opts.host);
        // let storage = ClientStorage::new(sled::open(&opts.database).unwrap());
        match &cmd {
            WalletCommand::Create(name) => self.create(name).await?,
            WalletCommand::Import(path) => self.import(path).await?,
            WalletCommand::Export(path) => self.export(path).await?,
            WalletCommand::Show => self.show().await?,
            WalletCommand::Open(name) => self.open(name).await?,
            WalletCommand::Help => self.help().await?,
            WalletCommand::Exit => {}
        };

        // Return processed command
        Ok(cmd)
    }

    async fn create(&self, name: &String) -> anyhow::Result<()> {
        // let wallet: Wallet = Wallet::new(&name);
        // prompt.show_wallet(&wallet);
        // storage.save(&name, &wallet)?
        Ok(())
    }

    async fn import(&self, path: &PathBuf) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn export(&self, path: &PathBuf) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn show(&self) -> anyhow::Result<()> {
        // let wallets: Vec<Wallet> = storage.load_all()?;
        // prompt.show_wallets(&wallets)
        Ok(())
    }

    // Run commands on an open wallet until user exits
    async fn open(&self, name: &String) -> anyhow::Result<()> {
        let handler = OpenWalletCommandHandler::new(
            self.host.clone(),
            self.database.clone(),
            self.version.clone(),
            name.clone(),
        );

        loop {
            let res_cmd = handler.handle().await;
            if res_cmd.is_ok() && res_cmd.unwrap() == OpenWalletCommand::Exit {
                return Ok(());
            }
        }
    }

    async fn help(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
