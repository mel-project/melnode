use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub(crate) enum ClientSubcommand {
    CreateWallet {
        wallet_name: String
    },
    Faucet {
        amount: String,
        unit: String
    },
    SendCoins {
        address: String,
        amount: String,
        unit: String
    },
    AddCoins {
        coin_id: String
    },
    // DepositCoins,
    // WithdrawCoins,
    // SwapCoins,
    ShowBalance,
    ShowWallets,
    Shell,
    Exit
}
