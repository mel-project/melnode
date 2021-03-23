// #[derive(Debug, PartialEq, EnumString)]
// #[strum(serialize_all = "snake_case")]
// enum OpenWalletCommand {
//     Faucet(FaucetArgs),
//     Deposit(DepositArgs),
//     Withdraw(WithdrawArgs),
//     Swap(SwagArgs),
//     Send(SendArgs),
//     Receive(ReceiveArgs),
//     Coins(CoinArgs),
//     Balance(BalanceArgs),
// }
//
// pub struct OpenWalletHandler {
//     client: ValClient,
//     storage: ClientStorage,
//     prompt: String
// }
//
// impl OpenWalletHandler {
//     pub(crate) fn new(client: ValClient, storage: ClientStorage, version: &str) -> Self {
//         let prompt_stack: Vec<String> = vec![format!("v{}", version).green().to_string()];
//         let prompt = format!("[client {}]% ", prompt_stack.join(" "));
//         Self {
//             client,
//             storage,
//             prompt,
//         }
//     }
//
//     pub(crate) async fn handle(&self) -> anyhow::Result<OpenWalletCommand> {
//         let input = WalletHandler::read_line(self.prompt.to_string()).await.unwrap();
//         let cmd = WalletCommand::from_str(input);
//         // Try to parse user input to select command
//         let res = self.try_parse(&input).await;
//         if res.is_err() {
//             Err("Could not parse command or command inputs")
//         }
//
//         // Process command
//         let cmd: OpenWalletCommand = res.unwrap();
//         // self.client.
//         match &cmd {
//             OpenWalletCommand::Faucet(_) => {}
//             OpenWalletCommand::Deposit(_) => {}
//             OpenWalletCommand::Withdraw(_) => {}
//             OpenWalletCommand::Swap(_) => {}
//             OpenWalletCommand::Send(_) => {}
//             OpenWalletCommand::Receive(_) => {}
//             OpenWalletCommand::Coins(_) => {}
//             OpenWalletCommand::Balance(_) => {}
//             // match opt {}
//
//             //flow pseudo-code - note should use ValClientSnapshot
//
//             // get trusted staker set,
//             // use height
//
//             // - swap
//             // 	- input pool, buy/sell, token name, denom, amount
//             // 		- just buy & sell abc
//             // 	- create tx
//             // 	- presend (do we sign here?)
//             // 	- send
//             // 	- query
//             // 	- print query results
//             // 	- update storage
//             // - send
//             // 	- input dest, amount, denom
//             // 	- create
//             // 	- presend (do we sign here?) / fee calc?
//             // 	- send
//             // 	- query / print query results
//             // 	- update storage
//             // - receive
//             // 	- input coin id
//             // 	- query
//             // 	- update storage
//             // - deposit / withdraw
//             // 	- input pool
//             // 	- input token
//             // 	- input amount (do we validate?)
//             // 	- create tx
//             // 	- prespend
//             // 	- send
//             // 	- query / print query results
//             // 	- update storage
//             // - faucet (enable-disable based on mainnet or not)
//             // 	- input receiver address, amount, denom, amount (upper bounded?)
//             // 	- create tx
//             // 	- presend
//             // 	- query
//             // 	- update storage
//             // - balance
//             // 	- load storage
//             // 	- print storage balance
//             // - coins
//             // 	- load storage
//             // 	- print storage coins
//         };
//
//         // Return which command was processed successfully
//         Ok(cmd)
//     }
//
//     async fn read_line(prompt: String) -> anyhow::Result<String> {
//         smol::unblock(move || {
//             let mut rl = rustyline::Editor::<()>::new();
//             Ok(rl.readline(&prompt)?)
//         }).await
//     }
//
//     async fn try_parse(&self, input: &String) -> anyhow::Result<WalletCommand> {
//         let x = input.split(' ').collect::<Vec<_>>().as_slice();
//     }
// }
//
// async fn handle_open_wallet_prompt() -> anyhow::Result<()> {
//     let prompt = OpenWalletPrompt::new();
//     let opt: OpenWalletCommand = prompt::handle_input();
//
//     let client = ValClient::new(opt.net_id, opt.remote);
//     let (height, _) = client.get_trusted_stakers();
//
//
// // }
// }
