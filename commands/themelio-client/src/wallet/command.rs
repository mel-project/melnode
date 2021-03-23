use std::path::PathBuf;
use std::str::FromStr;

use strum_macros::EnumString;

use colored::Colorize;
use nodeprot::ValClient;
use crate::storage::ClientStorage;
use crate::wallet::command::WalletCommand::Create;

#[derive(Eq, PartialEq, Debug, EnumString)]
#[strum(serialize_all = "snake_case")]
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
    client: ValClient,
    storage: ClientStorage,
    prompt: String
}

impl WalletCommandHandler {
    pub(crate) fn new(client: ValClient, storage: ClientStorage, version: &str) -> Self {
        let prompt_stack: Vec<String> = vec![format!("v{}", version).green().to_string()];
        let prompt = format!("[client wallet {}]% ", prompt_stack.join(" "));
        Self {
            client,
            storage,
            prompt,
        }
    }

    pub(crate) async fn handle(&self) -> anyhow::Result<WalletCommand> {
        // Parse input into a command
        let input = WalletCommandHandler::read_line(self.prompt.to_string()).await.unwrap();
        let cmd: WalletCommand = WalletCommand::from_str(&input)?;

        // Process command
        match &cmd {
            WalletCommand::Create(name) => {
                // let wallet: Wallet = Wallet::new(&name);
                // prompt.show_wallet(&wallet);
                // storage.save(&name, &wallet)?
            }
            WalletCommand::Show => {
                // let wallets: Vec<Wallet> = storage.load_all()?;
                // prompt.show_wallets(&wallets)
            }
            WalletCommand::Open(wallet) => {
                // let prompt_result = handle_open_wallet_prompt(&prompt, &storage).await?;
                // // handle res err if any
            }
            // WalletPromptOpt::ImportWallet(_import_path) => {}
            // WalletPromptOpt::ExportWallet(_export_path) => {}
            _ => {}
        };

        // Return processed command
        Ok(cmd)
    }

    async fn read_line(prompt: String) -> anyhow::Result<String> {
        smol::unblock(move || {
            let mut rl = rustyline::Editor::<()>::new();
            Ok(rl.readline(&prompt)?)
        }).await
    }

    // async fn try_parse(&self, input: &String) -> anyhow::Result<WalletCommand> {
    //     let x = input.split(' ').collect::<Vec<_>>().as_slice();
    // }
}
