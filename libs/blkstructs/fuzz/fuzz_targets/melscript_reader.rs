#![no_main]
use blkstructs::melvm::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let script = Covenant(data.to_vec());
    //println!("testing {}", hex::encode(&script.0));
    if let Some(ops) = script.to_ops() {
        //println!("{:?}", ops);
        let redone = Covenant::from_ops(&ops).unwrap();
        assert_eq!(redone, script);
    }
});
