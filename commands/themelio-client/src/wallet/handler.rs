use std::path::PathBuf;
use std::str::FromStr;
use strum_macros::EnumString;

use nodeprot::ValClient;
use crate::wallet::storage::ClientStorage;
use crate::wallet::handler::Command::CreateWallet;

#[derive(Debug, PartialEq, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum Command {
    CreateWallet(String),
    ImportWallet(PathBuf),
    ExportWallet(PathBuf),
    ShowWallets,
    OpenWallet(WalletData),
    Exit,
}

pub struct PromptHandler {
    client: ValClient,
    storage: ClientStorage,
    prompt: String
}

impl PromptHandler {
    pub(crate) fn new(client: ValClient, storage: ClientStorage, version: &str) -> Self {
        let prompt_stack: Vec<String> = vec![format!("v{}", version).green().to_string()];
        let prompt = format!("[client {}]% ", prompt_stack.join(" "));
        Self {
            client,
            storage,
            prompt,
        }
    }

    pub(crate) async fn handle(&self) -> anyhow::Result<Command> {
        let input = PromptHandler::read_line(self.prompt.to_string()).await.unwrap();
        let cmd = Command::from_str(input);
        // Try to parse user input to select command
        let res = self.try_parse(&input).await;
        if res.is_err() {
            Err("Could not parse command or command inputs")
        }

        // Process command
        let cmd = res.unwrap();
        match &cmd {
            Command::CreateWallet(name) => {
                let wallet: Wallet = Wallet::new(&name);
                prompt.show_wallet(&wallet);
                storage.save(&name, &wallet)?
            }
            Command::ShowWallets => {
                let wallets: Vec<Wallet> = storage.load_all()?;
                prompt.show_wallets(&wallets)
            }
            Command::OpenWallet(wallet) => {
                let prompt_result = handle_open_wallet_prompt(&prompt, &storage).await?;
                // handle res err if any
            }
            // WalletPromptOpt::ImportWallet(_import_path) => {}
            // WalletPromptOpt::ExportWallet(_export_path) => {}
            _ => {}
        };

        // Return which command was processed successfully
        Ok(cmd)
    }

    async fn read_line(prompt: String) -> anyhow::Result<String> {
        smol::unblock(move || {
            let mut rl = rustyline::Editor::<()>::new();
            Ok(rl.readline(&prompt)?)
        }).await
    }

    async fn try_parse(&self, input: &String) -> anyhow::Result<Command> {
        let x = input.split(' ').collect::<Vec<_>>().as_slice();
    }
}
