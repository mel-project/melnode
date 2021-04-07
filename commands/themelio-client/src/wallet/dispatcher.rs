use crate::wallet::command::WalletCommand;
use crate::wallet::open::command::OpenWalletCommand;
use crate::wallet::prompt::*;
use crate::wallet::wallet::Wallet;

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

            // Check if we errored or if user wants to exit
            match dispatch_result {
                Ok(cmd_res) => {
                    // Check whether to exit client prompt loop
                    if *cmd_res == WalletCommandResult::Exit {
                        return Ok(());
                    }
                }
                Err(err) => {
                    // Output command error
                    output_cmd_error(err, &cmd).await?;
                }
            }
        }
    }

    /// Parse user input into a wallet command process the command
    async fn dispatch(&self, cmd: &WalletCommand, open_cmd: &Option<OpenWalletCommand>) -> anyhow::Result<()> {
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
    async fn create(&self, name: &str) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let (sk, wallet_data) = wallet.create(name).await?;
        new_wallet_info(name, sk, &wallet_data);
        Ok(())
    }

    async fn delete(&self, name: &str) -> anyhow::Result<()> {
        todo!("Not implemented")
    }

    /// Shows all stored wallet names and the corresponding wallet address
    async fn show(&self) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);
        let wallets = wallet.get_all().await?;
        wallets_info(wallets).await;
        Ok(())
    }

    /// If wallet does not exist finish the open command,
    /// otherwise run commands in open wallet mode until exit command.
    async fn open(
        &self,
        name: &str,
        secret: &str,
    ) -> anyhow::Result<()> {
        let wallet = Wallet::new(&self.host, &self.database);

        let wallet_data = wallet.open(name, secret).await?;

        // wallet.load(name)? // error if it doesn't exist or secret doesn't match
        // returns wallet data ...

        // call run_open_wallet_prompt / run_active_wallet_prompt
        // with wallet data (this runs a loop and creates a dispatcher) (same pattern as main)

        // the result is WalletCommandResult::Open which has no params ... get invoked by inner Exit
        // ie WalletOpenCommandResult::Exit -> WalletCommandResult::Open with no params

        // note the inner loop in open is more complicated and makes more use of prompt and flow interactions...
        // that means WalletOpenCommandResult::* (all but exit) aren't relevant

        Ok(())
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
    ) -> anyhow::Result<()> {
       Ok(())
    }

    async fn help(&self) -> anyhow::Result<()> {
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

        Ok(())
    }

    async fn exit(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
