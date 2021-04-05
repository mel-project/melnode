use crate::wallet::command::{WalletCommand, WalletCommandResult};
use crate::wallet::open::command::OpenWalletCommand;
use crate::wallet::prompt::*;

pub struct Dispatcher {
    host: smol::net::SocketAddr,
    database: std::path::PathBuf,
    version: String
}

impl Dispatcher {
    pub(crate) fn new(host: &smol::net::SocketAddr, database: &std::path::PathBuf, version: &str) -> Self {
        let host = host.clone();
        let database = database.clone();
        let version = version.to_string();
        Self { host, database, version }
    }

    /// Dispatch commands from user input and show output using prompt until user exits.
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let prompt = format_prompt(&self.version).await;

        loop {
            // Get command from user input
            let (cmd, open_cmd) = input_command(&self.version).await?;

            // Dispatch the command
            let dispatch_result = &self.dispatch(&cmd, &open_cmd).await;

            // Handle the dispatch result
            match dispatch_result {
                Ok(cmd_res) => {
                    // Output command result
                    // output_cmd_result(&cmd_res).await?;

                    // Check whether to exit client prompt loop
                    if *cmd_res == WalletCommandResult::Exit {
                        return Ok(());
                    }
                }
                Err(err) => {
                    // Output command error
                    // output_dispatch_err(&err, &wallet_cmd);
                }
            }
        }
    }

    /// Parse user input into a wallet command process the command
    async fn dispatch(&self, cmd: &WalletCommand, open_cmd: &Option<OpenWalletCommand>) -> anyhow::Result<WalletCommandResult> {
        // Dispatch a command and return a command result
        match &cmd {
            WalletCommand::Create(name) => self.create(name).await,
            WalletCommand::Show => self.show().await,
            WalletCommand::Open(name, secret) => self.open(name, secret).await,
            WalletCommand::Use(name, secret) => self.use_(name, secret, open_cmd).await,
            WalletCommand::Delete(name) => self.delete(name).await,
            WalletCommand::Help => self.help().await,
            WalletCommand::Exit => { self.exit().await }
        }
    }

    /// Create wallet given a valid and unique name and store it
    async fn create(&self, name: &str) -> anyhow::Result<WalletCommandResult> {

        let wallet = Wallet::new(&self.host, &self.database);

        let command_result = wallet.create()?;
        // only alpha & name doesn't exist? then store and
        // return wallet name, address, & secret

        Ok(command_result)
        //
        // let (pk, sk) = wallet.create()?;
        //
        // let prompt = WalletPrompt::new();
        //
        // prompt.create_wallet_display(pk, sk)?;
        // let
        // let storage = WalletStorage::new(&self.database);
        // let wallet = Wallet::new(name)?;
        //
        //
        // // Check if wallet has only alphanumerics
        // if name.chars().all(char::is_alphanumeric) == false {
        //     eprintln!(
        //         ">> {}: wallet name can only contain alphanumerics",
        //         "ERROR".red().bold()
        //     );
        //     return Ok(());
        // }
        // // Check if wallet with same name already exits
        // if let Some(_stored_wallet_data) = storage.get_wallet_by_name(name).await? {
        //     eprintln!(
        //         ">> {}: wallet named '{}' already exists",
        //         "ERROR".red().bold(),
        //         &name
        //     );
        //     return Ok(());
        // }
        //
        // // Generate wallet data and store it
        // let (pk, sk) = tmelcrypt::ed25519_keygen();
        // let script = Covenant::std_ed25519_pk(pk);
        // let wallet_data = WalletData::new(script);
        // storage.insert_wallet(name, &wallet_data).await?;
        //
        // // Display contents of keypair and wallet data
        // let mut tw = TabWriter::new(vec![]);
        // writeln!(tw, ">> New data:\t{}", name.bold()).unwrap();
        // writeln!(
        //     tw,
        //     ">> Address:\t{}",
        //     wallet_data.my_script.hash().to_addr().yellow()
        // )
        //     .unwrap();
        // writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
        // eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());

        // Ok(())
    }

    async fn delete(&self, name: &str) -> anyhow::Result<WalletCommandResult> {
        todo!("Not implemented")
    }

    /// Shows all stored wallet names and the corresponding wallet address
    async fn show(&self) -> anyhow::Result<WalletCommandResult> {
        Ok(WalletCommandResult::Help)
        // let mut tw = TabWriter::new(vec![]);
        // writeln!(tw, ">> [NAME]\t[ADDRESS]")?;
        // let wallets = storage.get_all_wallets().await?;
        // for (name, wallet) in wallets.iter() {
        //     writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr())?;
        // }
        // tw.flush()?;
        // eprintln!("{}", String::from_utf8(tw.into_inner().unwrap()).unwrap());
        // Ok(())
    }

    /// If wallet does not exist finish the open command,
    /// otherwise run commands in open wallet mode until exit command.
    async fn open(
        &self,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<WalletCommandResult> {
        // init wallet

        // wallet.load(name)? // error if it doesn't exist or secret doesn't match
        // returns wallet data ...

        // call run_open_wallet_prompt / run_active_wallet_prompt
        // with wallet data (this runs a loop and creates a dispatcher) (same pattern as main)

        // the result is WalletCommandResult::Open which has no params ... get invoked by inner Exit
        // ie WalletOpenCommandResult::Exit -> WalletCommandResult::Open with no params

        // note the inner loop in open is more complicated and makes more use of prompt and flow interactions...
        // that means WalletOpenCommandResult::* (all but exit) aren't relevant

        Ok(WalletCommandResult::Help)
        // // Load wallet data from storage by name and make sure it exists.
        // let wallet = storage.get_wallet_by_name(name).await?;
        // if wallet.is_none() {
        //     eprintln!(
        //         ">> {}: wallet named '{}' does not exist in the database",
        //         "ERROR".red().bold(),
        //         &name
        //     );
        //     return Ok(());
        // }
        // let wallet = wallet.unwrap();
        //
        // // Verify the wallet secret correspond to the wallet address / public key
        // let wallet_secret = hex::decode(secret)?;
        // let wallet_secret = tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
        // if Covenant::std_ed25519_pk(wallet_secret.to_public()) != wallet.my_script {
        //     eprintln!(
        //         ">> {}: wallet named '{}' cannot be unlocked with this secret",
        //         "ERROR".red().bold(),
        //         &name
        //     );
        //     return Ok(());
        // }
        //
        // // Initialize open wallet command handler to handle transacting with wallet
        // let handler = OpenWalletCommandHandler::new(
        //     self.host,
        //     &self.version,
        //     name,
        //     secret,
        //     wallet,
        // );
        //
        // // Handle an command on an opened wallet.
        // // TODO: Likely better to store database variable in constructor
        // // than to pass the storage into handle, but both work.
        // loop {
        //     let res_cmd = handler.handle(&storage).await;
        //     if res_cmd.is_ok() && res_cmd.unwrap() == OpenWalletCommand::Exit {
        //         return Ok(());
        //     }
        // }
    }

    /// Use a particular wallet to run an open wallet command
    async fn use_(
        &self,
        name: &str,
        secret: &str,
        open_wallet_command: &Option<OpenWalletCommand>,
    ) -> anyhow::Result<WalletCommandResult> {
       Ok(WalletCommandResult::Help)
    }

    async fn help(&self) -> anyhow::Result<WalletCommandResult> {
        // eprintln!("\nAvailable commands are: ");
        // eprintln!(">> create <wallet-name>");
        // eprintln!(">> open <wallet-name> <secret>");
        // eprintln!(">> use <wallet-name> <secret> <open-wallet-args>");
        // eprintln!(">> show");
        // eprintln!(">> import <path>");
        // eprintln!(">> export <wallet-name> <path>");
        // eprintln!(">> help");
        // eprintln!(">> exit");
        // eprintln!(">> ");

        Ok(WalletCommandResult::Help)
    }

    async fn exit(&self) -> anyhow::Result<WalletCommandResult> {
        Ok(WalletCommandResult::Exit)
    }
}
