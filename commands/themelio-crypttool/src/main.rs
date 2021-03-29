use structopt::StructOpt;
#[derive(Debug, StructOpt)]
enum Args {
    /// Generate a ed25519 keypair
    GenerateEd25519,
}

fn print_header(hdr: &str) {
    eprintln!("===== {} =====", hdr);
}

fn main() {
    let args = Args::from_args();
    match args {
        Args::GenerateEd25519 => {
            print_header("NEW ED25519 KEYPAIR");
            let (pk, sk) = tmelcrypt::ed25519_keygen();
            eprintln!("PK = {}", hex::encode(pk.0));
            eprintln!("SK = {}", hex::encode(sk.0));
        }
    }
}
