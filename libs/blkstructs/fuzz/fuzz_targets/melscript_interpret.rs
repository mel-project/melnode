#![no_main]
use blkstructs::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (Vec<u8>, melvm::Covenant)| {
    let (txb, script) = data;
    let tx = rlp::decode(&txb);
    if let Ok(tx) = tx {
        assert_eq!(script.check(&tx), script.check(&tx));
    }
});
