use std::path::PathBuf;
use crate::wallet::storage::WalletStorage;

enum WalletPromptOpt {
    CreateWallet(String),
    ImportWallet(PathBuf),
    ExportWallet(PathBuf),
    ShowWallets,
    OpenWallet(WalletData),
}

pub async fn handle_prompt(prompt: &WalletPrompt, storage: &WalletStorage) -> anyhow::Result<()> {
    let opt: WalletPromptOpt = prompt::handle_input();
    match opt {
        WalletPromptOpt::CreateWallet(name) => {
            let wallet: Wallet = Wallet::new(&name);
            prompt.show_wallet(&wallet);
            storage.save(&name, &wallet)?
        }
        WalletPromptOpt::ShowWallets => {
            let wallets: Vec<Wallet> = storage.load_all()?;
            prompt.show_wallets(&wallets)
        }
        WalletPromptOpt::OpenWallet(wallet) => {
            let prompt_result = handle_open_wallet_prompt(&prompt, &storage).await?;
            // handle res err if any
        }
        // WalletPromptOpt::ImportWallet(_import_path) => {}
        // WalletPromptOpt::ExportWallet(_export_path) => {}
        _ => {}
    }
}
