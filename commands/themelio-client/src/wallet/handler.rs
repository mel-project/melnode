use std::path::PathBuf;
use crate::wallet::storage::WalletStorage;
use nodeprot::ValClient;

pub enum WalletCommand {
    CreateWallet(String),
    ImportWallet(PathBuf),
    ExportWallet(PathBuf),
    ShowWallets,
    OpenWallet(WalletData),
    Exit,
}

pub struct PromptHandler {
    client: ValClient,
    storage: WalletStorage,
    prompt: String
}

impl PromptHandler {
    pub(crate) fn new(client: ValClient, storage: WalletStorage, version: &str) -> Self {
        let prompt_stack: Vec<String> = vec![format!("v{}", version).green().to_string()];
        let prompt = format!("[anet client {}]% ", prompt_stack.join(" "));
        Self {
            client,
            storage,
            prompt,
        }
    }

    pub(crate) async fn handle(&self) -> anyhow::Result<WalletCommand> {
        let input = PromptHandler::read_line(self.prompt.to_string()).await.unwrap();

        // Try to parse user input to select command
        let res = self.try_parse(&input).await;
        if res.is_err() {
            Err("Could not parse command or command inputs")
        }

        // Process command
        let cmd = res.unwrap();
        match &cmd {
            WalletCommand::CreateWallet(name) => {
                let wallet: Wallet = Wallet::new(&name);
                prompt.show_wallet(&wallet);
                storage.save(&name, &wallet)?
            }
            WalletCommand::ShowWallets => {
                let wallets: Vec<Wallet> = storage.load_all()?;
                prompt.show_wallets(&wallets)
            }
            WalletCommand::OpenWallet(wallet) => {
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

    async fn try_parse(&self, input: &String) -> anyhow::Result<WalletCommand> {
        let x = input.split(' ').collect::<Vec<_>>().as_slice();
    }
}
