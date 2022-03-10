use async_compat::CompatExt;
use once_cell::sync::Lazy;

pub static PUBLIC_IP_ADDRESS: Lazy<String> = Lazy::new(|| {
    smol::future::block_on(public_ip::addr().compat()).expect("Could not obtain the public IP address.").to_string()
});