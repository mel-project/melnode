use std::convert::TryFrom;
use colored::Colorize;
use crate::wallet_shell::command::ShellCommand;
use crate::common::prompt::prompt::{InputPrompt, common_read_line};

pub struct InputPlainPrompt {}

impl InputPrompt<ShellCommand> for InputPlainPrompt {
    fn format_prompt(version: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
        format!("themelio-client").cyan().bold().to_string(),
        format!("(v{})", version).magenta().to_string(),
        format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    fn format_named_prompt(version: &str, name: &str) -> anyhow::Result<String> {
        todo!()
    }

    fn read_line(prompt: &str) -> anyhow::Result<ShellCommand> {
        let input = common_read_line(prompt.to_string())?;
        let wallet_cmd = ShellCommand::try_from(input)?;
        Ok(wallet_cmd)
    }
}