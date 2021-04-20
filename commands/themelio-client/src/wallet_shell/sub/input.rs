use std::convert::TryFrom;
use colored::Colorize;
use crate::common::prompt::prompt::{InputPrompt, common_read_line};
use crate::wallet_shell::sub::command::SubShellCommand;

pub struct InputPlainSubPrompt {}

impl InputPrompt<SubShellCommand> for InputPlainSubPrompt {
    fn format_prompt(_version: &str) -> anyhow::Result<String> {
        todo!("")
    }

    fn format_named_prompt(version: &str, name: &str) -> anyhow::Result<String> {
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
            format!("({})", name).cyan().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        Ok(format!("{}", prompt_stack.join(" ")))
    }

    fn read_line(prompt: &str) -> anyhow::Result<SubShellCommand> {
        let input = common_read_line(prompt.to_string())?;
        let open_wallet_cmd = SubShellCommand::try_from(input)?;
        Ok(open_wallet_cmd)
    }
}
