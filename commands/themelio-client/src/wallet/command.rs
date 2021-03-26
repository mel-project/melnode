use std::path::PathBuf;
use std::str::FromStr;

use strum_macros::EnumString;

use crate::wallet::common::read_line;
use crate::wallet::open::command::{OpenWalletCommand, OpenWalletCommandHandler};
use colored::Colorize;
use crate::wallet::data::WalletData;
use blkstructs::melvm::Covenant;
use crate::storage::ClientStorage;

#[derive(Eq, PartialEq, Debug, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum WalletCommand {
    Create(String),
    // Delete(String),
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
        let storage = ClientStorage::new(sled::open(&self.database).unwrap());
        match &cmd {
            WalletCommand::Create(name) => self.create(&storage, name).await?,
            WalletCommand::Import(path) => self.import(&storage, path).await?,
            WalletCommand::Export(path) => self.export(&storage, path).await?,
            WalletCommand::Show => self.show(&storage).await?,
            WalletCommand::Open(name) => self.open(&storage, name).await?,
            WalletCommand::Help => self.help().await?,
            WalletCommand::Exit => {}
        };

        // Return processed command
        Ok(cmd)
    }

    async fn create(&self, storage: &ClientStorage, name: &String) -> anyhow::Result<()> {
        use node::storage::SledMap;

        SledMap::new()
        use sled;
        NodeStorage::new(sled::open(&opt.database).unwrap(), testnet_genesis_config().await).share();
        sled.open()
        // Check if wallet with same name already exits
        if storage.get(&name);

        // Generate wallet data from keypair
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = Covenant::std_ed25519_pk(pk);
        let wallet_data = WalletData::new(script);

        // Display contents of keypair and wallet data

        // let wallet: Wallet = Wallet::new(&name);
        // prompt.show_wallet(&wallet);
        // storage.save(&name, &wallet)?
        Ok(())
    }

    async fn import(&self, storage: &ClientStorage, path: &PathBuf) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn export(&self, storage: &ClientStorage, path: &PathBuf) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn show(&self, storage: &ClientStorage) -> anyhow::Result<()> {
        // let wallets: Vec<Wallet> = storage.load_all()?;
        // prompt.show_wallets(&wallets)
        Ok(())
    }

    // Run commands on an open wallet until user exits
    async fn open(&self, storage: &ClientStorage, name: &String) -> anyhow::Result<()> {
        // Load wallet data from storage

        // Initialize open wallet command handler
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
