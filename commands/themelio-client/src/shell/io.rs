use anyhow::Error;

/// Helper method that read_line method in trait can call to handle raw user input.
pub(crate) async fn common_read_line(prompt: String) -> anyhow::Result<String> {
    smol::unblock(move || {
        let mut rl = rustyline::Editor::<()>::new();
        Ok(rl.readline(&prompt)?)
    })
    .await
}

/// Output the error when reading user input.
pub(crate) fn print_readline_error(_err: &Error) {
    eprintln!("ERROR: can't parse input command");
}
