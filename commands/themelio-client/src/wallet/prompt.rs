struct WalletPrompt {
    prompt: String
}

impl WalletPrompt {
    pub fn new(version: &str) -> Self {
        let prompt = WalletPrompt::new(version);
        let prompt_stack: Vec<String> = vec![
            format!("themelio-client").cyan().bold().to_string(),
            format!("(v{})", version).magenta().to_string(),
            format!("➜ ").cyan().bold().to_string(),
        ];
        let prompt = format!("{}", prompt_stack.join(" "));
        Self {
            prompt
        }
    }
}

