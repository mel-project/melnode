use std::convert::TryFrom;
use colored::Colorize;
use crate::common::prompt::prompt::{InputPrompt, common_read_line};
use crate::wallet_shell::sub::command::SubShellCommand;

use async_trait::async_trait;

pub struct SubShellInputPrompt {}

#[async_trait]
impl InputPrompt<SubShellCommand> for SubShellInputPrompt {
    async fn format_prompt(&self, _version: &str) -> anyhow::Result<String> {
        todo!("")
    }

    async fn format_named_prompt(&self, version: &str, name: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
            format!("({})", name).cyan().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    async fn read_line(&self, prompt: &str) -> anyhow::Result<SubShellCommand> {
        let input = common_read_line(prompt.to_string()).await?;
        let open_wallet_cmd = SubShellCommand::try_from(input)?;
        Ok(open_wallet_cmd)
    }
}
