use std::path::PathBuf;
use std::str::FromStr;

use strum_macros::{EnumString, ToString as StrumToString};

use crate::wallet::common::read_line;
use crate::wallet::open::command::{OpenWalletCommand, OpenWalletCommandHandler};
use colored::Colorize;
use crate::wallet::data::WalletData;
use blkstructs::melvm::Covenant;
use crate::storage::ClientStorage;
use tabwriter::TabWriter;

use std::io::prelude::*;

#[derive(Eq, PartialEq, Debug, EnumString, StrumToString)]
#[strum(serialize_all = "kebab-case")]
pub enum WalletCommand {
    Create(String),
    Delete(String),
    Import(PathBuf),
    Export(PathBuf),
    Show,
    Open(String),
    Help,
    Exit,
}

impl WalletCommand {
    /// Use strum to parse command and fill in input params
    pub fn parse_from_str(input: &String) -> anyhow::Result<WalletCommand> {
        let cmd: WalletCommand = WalletCommand::from_str(&input)?;
        let split_input: Vec<&str> = input.split_whitespace().collect();

        let cmd = match cmd {
            WalletCommand::Create(_) => {
                if split_input.len() != 2 {
                    anyhow::bail!("Invalid input params for wallet create");
                }
                WalletCommand::Create(split_input[1].to_string())
            }
            WalletCommand::Open(_) => {
                if split_input.len() != 2 {
                    anyhow::bail!("Invalid input params for wallet open");
                }
                WalletCommand::Open(split_input[1].to_string())
            }
            _ => { cmd }
        };

        Ok(cmd)
    }
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
        let input = input.unwrap();
        let cmd = WalletCommand::parse_from_str(&input)?;

        // Process command
        let storage = ClientStorage::new(&self.database);
        match &cmd {
            WalletCommand::Create(name) => self.create(&storage, name).await?,
            WalletCommand::Delete(name) => self.delete(&storage, name).await?,
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

        // Check if wallet with same name already exits
        if let Some(_stored_wallet_data) = storage.get_wallet_by_name(&name).await? {
            // display message
            eprintln!(">> {}: wallet data associated with that name already exists", "ERROR".red().bold());
            return Ok(());
        }

        // Generate wallet data from keypair and store it
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = Covenant::std_ed25519_pk(pk);
        let wallet_data = WalletData::new(script);
        storage.insert_wallet(&name, &wallet_data).await?;

        // Display contents of keypair and wallet data
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> New data:\t{}", name.bold()).unwrap();
        writeln!(
            tw,
            ">> Address:\t{}",
            wallet_data.my_script.hash().to_addr().yellow()
        )
        .unwrap();
        writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
        tw.flush().unwrap();

        Ok(())
    }

    async fn delete(&self, storage: &ClientStorage, name: &String) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }
    async fn import(&self, storage: &ClientStorage, path: &PathBuf) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn export(&self, storage: &ClientStorage, path: &PathBuf) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn show(&self, storage: &ClientStorage) -> anyhow::Result<()> {
        let wallets = storage.get_all_wallets().await?;
        // prompt.show_wallets(&wallets)
        Ok(())
    }

    /// If wallet does not exist finish the open command,
    /// otherwise run commands in open wallet mode until exit command.
    async fn open(&self, storage: &ClientStorage, name: &String) -> anyhow::Result<()> {
        // Load wallet data from storage
        let wallet = storage.get_wallet_by_name(&name).await?;
        if wallet.is_none() {
            // Display no wallet found and return
        }

        // Initialize open wallet command handler
        let handler = OpenWalletCommandHandler::new(
            self.host.clone(),
            self.version.clone(),
            name.clone(),
            wallet.unwrap(),
        );

        loop {
            let res_cmd = handler.handle().await;
            if res_cmd.is_ok() && res_cmd.unwrap() == OpenWalletCommand::Exit {
                return Ok(());
            }
        }
    }

    async fn help(&self) -> anyhow::Result<()> {
        eprintln!("\nAvailable commands are: ");
        eprintln!(">> create <wallet-name>");
        eprintln!(">> open <wallet-name> <secret>");
        eprintln!(">> show");
        eprintln!(">> import <path>");
        eprintln!(">> export <wallet-name> <path>");
        eprintln!(">> help");
        eprintln!(">> exit");
        Ok(())
    }
}
