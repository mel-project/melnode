mod constants;
mod melscript;
mod state;
mod transaction;
use afl::*;
use melscript::*;

fn main() {
    fuzz!(|data: &[u8]| {
        let script = Script(data.to_vec());
        if let Some(ops) = script.disassemble() {
            let redone = Script::assemble(&ops).unwrap();
            assert_eq!(redone, script);
        }
    })
}
