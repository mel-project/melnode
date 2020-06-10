#![no_main]
use blkstructs::melscript::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let script = Script(data.to_vec());
    //println!("testing {}", hex::encode(&script.0));
    if let Some(ops) = script.to_ops() {
        //println!("{:?}", ops);
        let redone = Script::from_ops(&ops).unwrap();
        assert_eq!(redone, script);
    }
});
