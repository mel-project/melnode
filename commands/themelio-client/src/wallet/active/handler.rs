
enum OpenWalletPromptOpt {
    Faucet(FaucetArgs),
    Deposit(DepositArgs),
    Withdraw(WithdrawArgs),
    Swap(SwagArgs),
    Send(SendArgs),
    Receive(ReceiveArgs),
    Coins(CoinArgs),
    Balance(BalanceArgs),
}

async fn handle_open_wallet_prompt() -> anyhow::Result<()> {
    let prompt = OpenWalletPrompt::new();
    let opt: OpenWalletPromptOpt = prompt::handle_input();

    let client = ValClient::new(opt.net_id, opt.remote);
    let (height, _) = client.get_trusted_stakers();

    // match opt {}

    //flow pseudo-code - note should use ValClientSnapshot

    // get trusted staker set,
    // use height

	// - swap
	// 	- input pool, buy/sell, token name, denom, amount
	// 		- just buy & sell abc
	// 	- create tx
	// 	- presend (do we sign here?)
	// 	- send
	// 	- query
	// 	- print query results
	// 	- update storage
	// - send
	// 	- input dest, amount, denom
	// 	- create
	// 	- presend (do we sign here?) / fee calc?
	// 	- send
	// 	- query / print query results
	// 	- update storage
	// - receive
	// 	- input coin id
	// 	- query
	// 	- update storage
	// - deposit / withdraw
	// 	- input pool
	// 	- input token
	// 	- input amount (do we validate?)
	// 	- create tx
	// 	- prespend
	// 	- send
	// 	- query / print query results
	// 	- update storage
	// - faucet (enable-disable based on mainnet or not)
	// 	- input receiver address, amount, denom, amount (upper bounded?)
	// 	- create tx
	// 	- presend
	// 	- query
	// 	- update storage
	// - balance
	// 	- load storage
	// 	- print storage balance
	// - coins
	// 	- load storage
	// 	- print storage coins

// }
}
