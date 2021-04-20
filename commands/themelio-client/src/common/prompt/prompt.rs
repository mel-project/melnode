pub trait InputPrompt<T> {
    /// Format the a prompt with the version of the binary.
    fn format_prompt(version: &str) -> anyhow::Result<String>;

    /// Format the a prompt with the version of the binary and a name for the prompt.
    fn format_named_prompt(version: &str, name: &str) -> anyhow::Result<String>;

    /// Get user input and parse it into a wallet_shell command.
    fn read_line(prompt: &str) -> anyhow::Result<T>;


}

/// Helper method that read_line method in trait can call to handle raw user input.
pub fn common_read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
        .await
}
