use blkstructs::melvm::Covenant;
#[cfg(fuzzing)]
use honggfuzz::fuzz;

#[cfg(fuzzing)]
fn main() {
    use env_logger::Env;
    env_logger::Builder::from_env(Env::default().default_filter_or("blkstructs")).init();
    // Here you can parse `std::env::args and
    // setup / initialize your project

    // You have full control over the loop but
    // you're supposed to call `fuzz` ad vitam aeternam
    loop {
        // The fuzz macro gives an arbitrary object (see `arbitrary crate`)
        // to a closure-like block of code.
        // For performance reasons, it is recommended that you use the native type
        // `&[u8]` when possible.
        // Here, this slice will contain a "random" quantity of "random" data.
        fuzz!(|data: &[u8]| { test_once(data) });
    }
}

fn test_once(data: &[u8]) {
    let covenant = Covenant(data.to_vec());
    if let Some(weight) = covenant.weight() {
        if weight > 100000 {
            return;
        }
    }
    covenant.check_raw(&[]);
    if let Some(ops) = covenant.to_ops() {
        assert_eq!(Covenant::from_ops(&ops).unwrap(), covenant);
    }
}

#[cfg(not(fuzzing))]
fn main() {}
