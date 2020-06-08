#![no_main]
use blkstructs::melscript::*;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let script = Script(data.to_vec());
    if let Some(ops) = script.disassemble() {
        //println!("{:?}", ops);
        let redone = Script::assemble(&ops).unwrap();
        assert_eq!(redone, script);
    }
});
