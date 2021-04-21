use crate::common::prompt::prompt::{common_read_line, InputPrompt};
use crate::wallet_shell::command::ShellCommand;
use async_trait::async_trait;
use colored::Colorize;
use std::convert::TryFrom;

pub struct ShellInputPrompt {}

impl ShellInputPrompt {
    pub fn new() -> Self {
        return ShellInputPrompt {};
    }
}

#[async_trait]
impl InputPrompt<ShellCommand> for ShellInputPrompt {
    async fn format_prompt(&self, version: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    async fn format_named_prompt(&self, version: &str, name: &str) -> anyhow::Result<String> {
        todo!()
    }

    async fn read_line(&self, prompt: &str) -> anyhow::Result<ShellCommand> {
        let input = common_read_line(prompt.to_string()).await?;
        let wallet_cmd = ShellCommand::try_from(input)?;
        Ok(wallet_cmd)
    }
}
