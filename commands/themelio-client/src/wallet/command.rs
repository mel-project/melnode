use crate::storage::ClientStorage;
use crate::wallet::common::read_line;
use crate::wallet::data::WalletData;
use crate::wallet::open::command::{OpenWalletCommand, OpenWalletCommandHandler};
use blkstructs::melvm::Covenant;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use tabwriter::TabWriter;

use serde_scan::ScanError;
use std::convert::{TryFrom, TryInto};
use std::io::prelude::*;

#[derive(Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WalletCommand {
    Create(String),
    Delete(String),
    Import(String),
    Export(String),
    Show,
    Open(String, String),
    Use(String, String),
    Help,
    Exit,
}

impl TryFrom<String> for WalletCommand {
    type Error = ScanError;

    /// Uses serde scan internally to parse a whitespace delimited string into a command
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let cmd: Result<WalletCommand, _> = serde_scan::from_str(&value);
        cmd
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
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        let prompt = format!("{}", prompt_stack.join(" "));
        Self {
            host,
            database,
            version,
            prompt,
        }
    }

    /// Parse user input into a wallet command process the command
    pub(crate) async fn handle(&self) -> anyhow::Result<WalletCommand> {
        // Convert user input into a command
        let input = read_line(self.prompt.to_string()).await?;
        let (wallet_cmd, open_wallet_cmd) = self.get_commands(
            &input[..]
        ).await?;

        // Process the command
        let storage = ClientStorage::new(&self.database);
        match &wallet_cmd {
            WalletCommand::Create(wallet_name) => self.create(&storage, wallet_name).await?,
            WalletCommand::Delete(wallet_name) => self.delete(&storage, wallet_name).await?,
            WalletCommand::Import(import_path) => self.import(&storage, import_path).await?,
            WalletCommand::Export(export_path) => self.export(&storage, export_path).await?,
            WalletCommand::Show => self.show(&storage).await?,
            WalletCommand::Open(wallet_name, secret) => {
                self.open_wallet(&storage, wallet_name, secret).await?
            },
            WalletCommand::Use(wallet_name, secret) => {
                self.use_wallet(&storage, wallet_name, secret, &open_wallet_cmd.unwrap()).await?
            }
            WalletCommand::Help => self.help().await?,
            WalletCommand::Exit => { }
        };

        // Return processed command
        Ok(wallet_cmd)
    }

    /// Given the user input parse it into a wallet and (if applicable) open wallet command
    async fn get_commands(&self, input: &str) -> anyhow::Result<(WalletCommand, Option<OpenWalletCommand>)> {
        if input.starts_with("wallet-use") {
            let args: Vec<String> = input.split(" ").map(|s| s.to_string()).collect();
            let (left, right): (&str, &str) = (&args[0..2].join(" "), &args[2..].join(" "));
            let wallet_cmd = WalletCommand::try_from(left.to_string())?;
            let open_wallet_cmd = OpenWalletCommand::try_from(right.to_string())?;
            Ok((wallet_cmd, Some(open_wallet_cmd)))
        } else {
            let wallet_cmd = WalletCommand::try_from(input.to_string())?;
            Ok((wallet_cmd, None))
        }
    }

    /// Create wallet data by name and store it if its valid and does not already exist
    async fn create(&self, storage: &ClientStorage, name: &str) -> anyhow::Result<()> {
        // Check if wallet has only alphanumerics
        if name.chars().all(char::is_alphanumeric) == false {
            eprintln!(
                ">> {}: wallet name can only contain alphanumerics",
                "ERROR".red().bold()
            );
            return Ok(());
        }
        // Check if wallet with same name already exits
        if let Some(_stored_wallet_data) = storage.get_wallet_by_name(name).await? {
            eprintln!(
                ">> {}: wallet named '{}' already exists",
                "ERROR".red().bold(),
                &name
            );
            return Ok(());
        }

        // Generate wallet data and store it
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        let script = Covenant::std_ed25519_pk(pk);
        let wallet_data = WalletData::new(script);
        storage.insert_wallet(name, &wallet_data).await?;

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
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());

        Ok(())
    }

    async fn delete(&self, storage: &ClientStorage, name: &str) -> anyhow::Result<()> {
        todo!("Not implemented")
    }
    async fn import(&self, storage: &ClientStorage, path: &str) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    async fn export(&self, storage: &ClientStorage, path: &str) -> anyhow::Result<()> {
        anyhow::bail!("Not Implemented")
    }

    /// Shows all stored wallet names and the corresponding wallet address
    async fn show(&self, storage: &ClientStorage) -> anyhow::Result<()> {
        let mut tw = TabWriter::new(vec![]);
        writeln!(tw, ">> [NAME]\t[ADDRESS]")?;
        let wallets = storage.get_all_wallets().await?;
        for (name, wallet) in wallets.iter() {
            writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr())?;
        }
        tw.flush()?;
        eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        Ok(())
    }

    /// If wallet does not exist finish the open command,
    /// otherwise run commands in open wallet mode until exit command.
    async fn open_wallet(
        &self,
        storage: &ClientStorage,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<()> {
        // Load wallet data from storage by name and make sure it exists.
        let wallet = storage.get_wallet_by_name(name).await?;
        if wallet.is_none() {
            eprintln!(
                ">> {}: wallet named '{}' does not exist in the database",
                "ERROR".red().bold(),
                &name
            );
            return Ok(());
        }
        let wallet = wallet.unwrap();

        // Verify the wallet secret correspond to the wallet address / public key
        let wallet_secret = hex::decode(secret)?;
        let wallet_secret = tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
        if Covenant::std_ed25519_pk(wallet_secret.to_public()) != wallet.my_script {
            eprintln!(
                ">> {}: wallet named '{}' cannot be unlocked with this secret",
                "ERROR".red().bold(),
                &name
            );
            return Ok(());
        }

        // Initialize open wallet command handler to handle transacting with wallet
        let handler = OpenWalletCommandHandler::new(
            self.host.clone(),
            self.version.clone(),
            name.clone(),
            secret.clone(),
            wallet,
        );

        // Handle an command on an opened wallet.
        // TODO: Likely better to store database variable in constructor
        // than to pass the storage into handle, but both work.
        loop {
            let res_cmd = handler.handle(&storage).await;
            if res_cmd.is_ok() && res_cmd.unwrap() == OpenWalletCommand::Exit {
                return Ok(());
            }
        }
    }

    /// Use a particular wallet to run an open wallet command
    async fn use_wallet(
        &self,
        storage: &ClientStorage,
        name: &str,
        secret: &str,
        open_wallet_command: &OpenWalletCommand,
    ) -> anyhow::Result<()> {
        Ok(())
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
