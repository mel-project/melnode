use async_compat::CompatExt;
use once_cell::sync::Lazy;

// like smol::future::block_on(some_future.compat()). Of course that's a trait that you must use: use async_compat::CompatExt;
// [2:18 PM]


pub static PUBLIC_IP_ADDRESS: Lazy<String> = Lazy::new(|| {
    smol::future::block_on(public_ip::addr().compat()).expect("Could not obtain the public IP address.").to_string()
});