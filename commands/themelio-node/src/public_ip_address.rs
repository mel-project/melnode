use once_cell::sync::Lazy;

pub static PUBLIC_IP_ADDRESS: Lazy<String> = Lazy::new(|| {
    smol::future::block_on(public_ip::addr()).expect("Could not obtain the public IP address.").to_string()
});