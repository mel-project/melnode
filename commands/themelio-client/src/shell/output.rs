use anyhow::Error;

use crate::shell::command::ShellCommand;

/// Output the error when dispatching command.
pub(crate) fn print_command_error(err: &Error, cmd: &ShellCommand) {
    eprintln!("ERROR: {} with wallet_shell command {:?}", err, cmd);
}

/// Output the error when reading user input.
pub(crate) fn print_readline_error(_err: &Error) {
    eprintln!("ERROR: can't parse input command");
}

/// Show exit message.
pub(crate) fn print_exit_message() {
    eprintln!("\nExiting Themelio Client wallet_shell");
}
