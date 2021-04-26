use std::convert::TryFrom;

use async_trait::async_trait;
use colored::Colorize;

use crate::shell::command::ShellCommand;
use crate::utils::prompt::{common_read_line, InputPrompt};
use crate::wallet::error::WalletError;

pub struct ShellInputPrompt;

#[async_trait]
impl InputPrompt<ShellCommand> for ShellInputPrompt {
    async fn format_prompt(&self, version: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            "themelio-client".to_string().cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            "➜ ".to_string().cyan().bold().to_string(),
        ];
        Ok(prompt_stack.join(" "))
    }

    async fn format_named_prompt(&self, version: &str, name: &str) -> anyhow::Result<String> {
        todo!()
    }

    async fn read_command(&self, prompt: &str) -> anyhow::Result<ShellCommand> {
        let input = common_read_line(prompt.to_string()).await?;
        let wallet_cmd = ShellCommand::try_from(input.clone());
        if wallet_cmd.is_err() {
            anyhow::bail!(WalletError::InvalidInputArgs(input))
        }
        Ok(wallet_cmd?)
    }
}
