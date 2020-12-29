pub struct AvailableWallets {}

impl AvailableWallets {
    pub fn add() {
        // if wallets.get(&wallet_name.to_string()).is_some() {
        //     eprintln!(">> {}: data already exists", "ERROR".red().bold());
        //     continue;
        // }
        // let (pk, sk) = tmelcrypt::ed25519_keygen();
        // let script = melscript::Script::std_ed25519_pk(pk);
        // let wallet = WalletData::new(script.clone());
        // wallets.insert(wallet_name.to_string(), wallet.clone());
        // writeln!(tw, ">> New data:\t{}", wallet_name.bold()).unwrap();
        // writeln!(tw, ">> Address:\t{}", script.hash().to_addr().yellow()).unwrap();
        // writeln!(tw, ">> Secret:\t{}", hex::encode(sk.0).dimmed()).unwrap();
        // tw.flush().unwrap();
        // let wallet_record = WalletRecord::new(wallet, wallet_name);
        //
        // // Insert data record
        // let conn = Connection::open_in_memory();
        // wallet_record.store(&conn.unwrap()).expect("SQL error?");
    }

    pub fn unlock() {
        // if let Some(wallet) = wallets.get(&wallet_name.to_string()) {
        //     let wallet_secret = hex::decode(wallet_secret)?;
        //     let wallet_secret =
        //         tmelcrypt::Ed25519SK(wallet_secret.as_slice().try_into()?);
        //     if melscript::Script::std_ed25519_pk(wallet_secret.to_public())
        //         != wallet.my_script
        //     {
        //         Err(anyhow::anyhow!(
        //             "unlocking failed, make sure you have the right secret!"
        //         ))?;
        //     }
        //     current_wallet = Some((wallet_name.to_string(), wallet_secret));
        //     prompt_stack.push(format!("({})", wallet_name).yellow().to_string());
        // }
    }

    pub fn list() {
        // writeln!(tw, ">> [NAME]\t[ADDRESS]")?;
        // for (name, wallet) in wallets.iter() {
        //     writeln!(tw, ">> {}\t{}", name, wallet.my_script.hash().to_addr())?;
        // }
    }
}
