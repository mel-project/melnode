use crate::transaction as txn;
use arbitrary::Arbitrary;
use rlp::{Decodable, Encodable};
use std::collections::HashMap;
use std::convert::TryInto;

#[derive(Clone, Eq, PartialEq, Debug, Arbitrary)]
pub struct Script(pub Vec<u8>);

impl Script {
    pub fn check(&self, tx: &txn::Transaction) -> bool {
        self.check_opt(tx).is_some()
    }

    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&self.0)
    }

    fn check_opt(&self, tx: &txn::Transaction) -> Option<()> {
        let txb = rlp::encode(tx);
        let tx_val: Value = rlp::decode(&txb).unwrap();
        let ops = self.to_ops()?;
        let mut hm = HashMap::new();
        hm.insert(0, tx_val);
        hm.insert(1, Value::from_bytes(&tx.hash_nosigs().0));
        Executor::new(hm).run_return(&ops)
    }

    pub fn std_ed25519_pk(pk: tmelcrypt::Ed25519PK) -> Self {
        Script::from_ops(&[
            OpCode::PUSHI(0.into()),
            OpCode::PUSHI(6.into()),
            OpCode::LOADIMM(0),
            OpCode::VREF,
            OpCode::VREF,
            OpCode::PUSHB(pk.0.to_vec()),
            OpCode::LOADIMM(1),
            OpCode::SIGEOK,
        ])
        .unwrap()
    }

    pub fn from_ops(ops: &[OpCode]) -> Option<Self> {
        let mut output: Vec<u8> = Vec::new();
        // go through output
        for op in ops {
            Script::assemble_one(op, &mut output)?
        }
        Some(Script(output))
    }

    pub fn always_true() -> Self {
        Script::from_ops(&[OpCode::PUSHI(1.into())]).unwrap()
    }

    fn disassemble_one(
        bcode: &mut Vec<u8>,
        output: &mut Vec<OpCode>,
        rec_depth: usize,
    ) -> Option<()> {
        if rec_depth > 16 {
            return None;
        }
        let u16arg = |vec: &mut Vec<u8>| {
            let mut z = [0; 2];
            z[0] = vec.pop()?;
            z[1] = vec.pop()?;
            Some(u16::from_be_bytes(z))
        };
        match bcode.pop()? {
            // arithmetic
            0x10 => output.push(OpCode::ADD),
            0x11 => output.push(OpCode::SUB),
            0x12 => output.push(OpCode::MUL),
            0x13 => output.push(OpCode::DIV),
            0x14 => output.push(OpCode::REM),
            // logic
            0x20 => output.push(OpCode::AND),
            0x21 => output.push(OpCode::OR),
            0x22 => output.push(OpCode::XOR),
            0x23 => output.push(OpCode::NOT),
            0x24 => output.push(OpCode::EQL),
            // cryptography
            0x30 => output.push(OpCode::HASH),
            //0x31 => output.push(OpCode::SIGE),
            0x32 => output.push(OpCode::SIGEOK),
            // storage
            0x40 => output.push(OpCode::LOAD),
            0x41 => output.push(OpCode::STORE),
            0x42 => output.push(OpCode::LOADIMM(u16arg(bcode)?)),
            0x43 => output.push(OpCode::STOREIMM(u16arg(bcode)?)),
            // vectors
            0x50 => output.push(OpCode::VREF),
            0x51 => output.push(OpCode::VAPPEND),
            0x52 => output.push(OpCode::VEMPTY),
            0x53 => output.push(OpCode::VLENGTH),
            0x54 => output.push(OpCode::VPUSH),
            0x55 => output.push(OpCode::VSLICE),
            0x56 => output.push(OpCode::BEMPTY),
            // control flow
            0xa0 => output.push(OpCode::JMP(u16arg(bcode)?)),
            0xa1 => output.push(OpCode::BEZ(u16arg(bcode)?)),
            0xa2 => output.push(OpCode::BNZ(u16arg(bcode)?)),
            0xb0 => {
                let iterations = u16arg(bcode)?;
                let count = u16arg(bcode)?;
                let mut rec_output = Vec::new();
                for _ in 0..count {
                    Script::disassemble_one(bcode, &mut rec_output, rec_depth + 1)?;
                }
                output.push(OpCode::LOOP(iterations, rec_output));
            }
            // literals
            0xf0 => {
                let strlen = bcode.pop()?;
                let mut blit = Vec::with_capacity(strlen as usize);
                for _ in 0..strlen {
                    blit.push(bcode.pop()?);
                }
                output.push(OpCode::PUSHB(blit))
            }
            0xf1 => {
                let mut buf = [0; 32];
                for r in buf.iter_mut() {
                    *r = bcode.pop()?
                }
                output.push(OpCode::PUSHI(bigint::U256::from_big_endian(&buf)))
            }
            _ => return None,
        }
        Some(())
    }

    pub fn to_ops(&self) -> Option<Vec<OpCode>> {
        // reverse to make it a poppable stack
        let mut reversed = self.0.clone();
        reversed.reverse();
        let mut output = Vec::new();
        while !reversed.is_empty() {
            Script::disassemble_one(&mut reversed, &mut output, 0)?
        }
        Some(output)
    }

    fn assemble_one(op: &OpCode, output: &mut Vec<u8>) -> Option<()> {
        match op {
            // arithmetic
            OpCode::ADD => output.push(0x10),
            OpCode::SUB => output.push(0x11),
            OpCode::MUL => output.push(0x12),
            OpCode::DIV => output.push(0x13),
            OpCode::REM => output.push(0x14),
            // logic
            OpCode::AND => output.push(0x20),
            OpCode::OR => output.push(0x21),
            OpCode::XOR => output.push(0x22),
            OpCode::NOT => output.push(0x23),
            OpCode::EQL => output.push(0x24),
            // cryptography
            OpCode::HASH => output.push(0x30),
            //OpCode::SIGE => output.push(0x31),
            OpCode::SIGEOK => output.push(0x32),
            // storage
            OpCode::LOAD => output.push(0x40),
            OpCode::STORE => output.push(0x41),
            OpCode::LOADIMM(idx) => {
                output.push(0x42);
                output.extend_from_slice(&idx.to_be_bytes());
            }
            OpCode::STOREIMM(idx) => {
                output.push(0x43);
                output.extend_from_slice(&idx.to_be_bytes());
            }
            // vectors
            OpCode::VREF => output.push(0x50),
            OpCode::VAPPEND => output.push(0x51),
            OpCode::VEMPTY => output.push(0x52),
            OpCode::VLENGTH => output.push(0x53),
            OpCode::VPUSH => output.push(0x54),
            OpCode::VSLICE => output.push(0x55),
            OpCode::BEMPTY => output.push(0x56),
            // control flow
            OpCode::JMP(val) => {
                output.push(0xa0);
                output.extend_from_slice(&val.to_be_bytes());
            }
            OpCode::BEZ(val) => {
                output.push(0xa1);
                output.extend_from_slice(&val.to_be_bytes());
            }
            OpCode::BNZ(val) => {
                output.push(0xa2);
                output.extend_from_slice(&val.to_be_bytes());
            }
            OpCode::LOOP(iterations, ops) => {
                output.push(0xb0);
                output.extend_from_slice(&iterations.to_be_bytes());
                let op_cnt: u16 = ops.len().try_into().ok()?;
                output.extend_from_slice(&op_cnt.to_be_bytes());
                for op in ops {
                    Script::assemble_one(op, output)?
                }
            }
            // literals
            OpCode::PUSHB(bts) => {
                output.push(0xf0);
                if bts.len() > 255 {
                    return None;
                }
                output.push(bts.len() as u8);
                output.extend_from_slice(bts);
            }
            OpCode::PUSHI(num) => {
                output.push(0xf1);
                let mut out = [0; 32];
                num.to_big_endian(&mut out);
                output.extend_from_slice(&out);
            }
        }
        Some(())
    }
}

impl Encodable for Script {
    fn rlp_append(&self, s: &mut rlp::RlpStream) {
        (self.0).rlp_append(s)
    }
}

impl Decodable for Script {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let raw = Vec::decode(rlp)?;
        Ok(Script(raw))
    }
}

struct Executor {
    stack: Vec<Value>,
    heap: HashMap<u16, Value>,
}

impl Executor {
    fn new(heap_init: HashMap<u16, Value>) -> Self {
        Executor {
            stack: Vec::new(),
            heap: heap_init,
        }
    }
    fn do_triop(&mut self, op: impl Fn(Value, Value, Value) -> Option<Value>) -> Option<()> {
        let stack = &mut self.stack;
        let x = stack.pop()?;
        let y = stack.pop()?;
        let z = stack.pop()?;
        stack.push(op(x, y, z)?);
        Some(())
    }
    fn do_binop(&mut self, op: impl Fn(Value, Value) -> Option<Value>) -> Option<()> {
        let stack = &mut self.stack;
        let x = stack.pop()?;
        let y = stack.pop()?;
        stack.push(op(x, y)?);
        Some(())
    }
    fn do_monop(&mut self, op: impl Fn(Value) -> Option<Value>) -> Option<()> {
        let stack = &mut self.stack;
        let x = stack.pop()?;
        stack.push(op(x)?);
        Some(())
    }
    fn do_op(&mut self, op: &OpCode, pc: u32) -> Option<u32> {
        match op {
            // arithmetic
            OpCode::ADD => {
                self.do_binop(|x, y| Some(Value::Int(x.as_int()?.overflowing_add(y.as_int()?).0)))?
            }
            OpCode::SUB => {
                self.do_binop(|x, y| Some(Value::Int(x.as_int()?.overflowing_sub(y.as_int()?).0)))?
            }
            OpCode::MUL => {
                self.do_binop(|x, y| Some(Value::Int(x.as_int()?.overflowing_mul(y.as_int()?).0)))?
            }
            OpCode::DIV => self.do_binop(|x, y| {
                if y.as_int()? == bigint::U256::zero() {
                    None
                } else {
                    Some(Value::Int(x.as_int()?.overflowing_div(y.as_int()?).0))
                }
            })?,
            OpCode::REM => self.do_binop(|x, y| {
                if y.as_int()? == bigint::U256::zero() {
                    None
                } else {
                    Some(Value::Int(x.as_int()?.overflowing_rem(y.as_int()?).0))
                }
            })?,
            // logic
            OpCode::AND => self.do_binop(|x, y| Some(Value::Int(x.as_int()? & y.as_int()?)))?,
            OpCode::OR => self.do_binop(|x, y| Some(Value::Int(x.as_int()? | y.as_int()?)))?,
            OpCode::XOR => self.do_binop(|x, y| Some(Value::Int(x.as_int()? ^ y.as_int()?)))?,
            OpCode::NOT => self.do_monop(|x| Some(Value::Int(!x.as_int()?)))?,
            OpCode::EQL => self.do_binop(|x, y| {
                let x = x.as_int()?;
                let y = y.as_int()?;
                if x == y {
                    Some(Value::Int(bigint::U256::one()))
                } else {
                    Some(Value::Int(bigint::U256::zero()))
                }
            })?,
            // cryptography
            OpCode::HASH => self.do_monop(|to_hash| {
                let to_hash = to_hash.as_bytes()?;
                let hash = tmelcrypt::hash_single(&to_hash);
                Some(Value::from_bytes(&hash.0))
            })?,
            OpCode::SIGEOK => self.do_triop(|message, public_key, signature| {
                //println!("SIGEOK({:?}, {:?}, {:?})", message, public_key, signature);
                let public_key = tmelcrypt::Ed25519PK::from_bytes(&public_key.as_bytes()?)?;
                Some(Value::from_bool(
                    public_key.verify(&message.as_bytes()?, &signature.as_bytes()?),
                ))
            })?,
            // storage access
            OpCode::STORE => {
                let addr = self.stack.pop()?.as_u16()?;
                let val = self.stack.pop()?;
                self.heap.insert(addr, val);
            }
            OpCode::LOAD => {
                let addr = self.stack.pop()?.as_u16()?;
                let res = self.heap.get(&addr)?.clone();
                self.stack.push(res)
            }
            OpCode::STOREIMM(idx) => {
                let val = self.stack.pop()?;
                self.heap.insert(*idx, val);
            }
            OpCode::LOADIMM(idx) => {
                let res = self.heap.get(idx)?.clone();
                self.stack.push(res)
            }
            // vector operations
            OpCode::VREF => self.do_binop(|vec, idx| {
                let idx = idx.as_u16()? as usize;
                match vec {
                    Value::Bytes(bts) => Some(Value::Int(bigint::U256::from(*bts.get(idx)?))),
                    Value::Vector(elems) => Some(elems.get(idx)?.clone()),
                    _ => None,
                }
            })?,
            OpCode::VAPPEND => self.do_binop(|v1, v2| match (v1, v2) {
                (Value::Bytes(mut v1), Value::Bytes(v2)) => {
                    v1.append(v2);
                    Some(Value::Bytes(v1))
                }
                (Value::Vector(mut v1), Value::Vector(v2)) => {
                    v1.append(v2);
                    Some(Value::Vector(v1))
                }
                _ => None,
            })?,
            OpCode::VSLICE => self.do_triop(|vec, i, j| {
                let i = i.as_u16()? as usize;
                let j = j.as_u16()? as usize;
                match vec {
                    Value::Vector(mut vec) => Some(Value::Vector(vec.slice(i..j))),
                    Value::Bytes(mut vec) => Some(Value::Bytes(vec.slice(i..j))),
                    _ => None,
                }
            })?,
            OpCode::VLENGTH => self.do_monop(|vec| match vec {
                Value::Vector(vec) => Some(Value::Int(bigint::U256::from(vec.len()))),
                Value::Bytes(vec) => Some(Value::Int(bigint::U256::from(vec.len()))),
                _ => None,
            })?,
            OpCode::VEMPTY => self.stack.push(Value::Vector(im_rc::Vector::new())),
            OpCode::BEMPTY => self.stack.push(Value::Bytes(im_rc::Vector::new())),
            OpCode::VPUSH => self.do_binop(|vec, val| match vec {
                Value::Vector(mut vec) => {
                    vec.push_back(val);
                    Some(Value::Vector(vec))
                }
                Value::Bytes(mut vec) => {
                    let bts = val.as_int()?;
                    if bts > bigint::U256::from(255) {
                        return None;
                    }
                    let bts = bts.low_u32() as u8;
                    vec.push_back(bts);
                    Some(Value::Bytes(vec))
                }
                _ => None,
            })?,
            // control flow
            OpCode::BEZ(jgap) => {
                let top = self.stack.pop()?;
                self.stack.push(top.clone());
                if top == Value::Int(bigint::U256::zero()) {
                    return Some(pc + 1 + *jgap as u32);
                }
            }
            OpCode::BNZ(jgap) => {
                let top = self.stack.pop()?;
                self.stack.push(top.clone());
                if top != Value::Int(bigint::U256::zero()) {
                    return Some(pc + 1 + *jgap as u32);
                }
            }
            OpCode::JMP(jgap) => return Some(pc + 1 + *jgap as u32),
            OpCode::LOOP(iterations, ops) => {
                for _ in 0..*iterations {
                    self.run_bare(&ops)?
                }
            }
            // literals
            OpCode::PUSHB(bts) => {
                let bts = Value::from_bytes(bts);
                self.stack.push(bts);
            }
            OpCode::PUSHI(num) => self.stack.push(Value::Int(*num)),
        }
        Some(pc + 1)
    }
    fn run_bare(&mut self, ops: &[OpCode]) -> Option<()> {
        assert!(ops.len() < 512 * 1024);
        let mut pc = 0;
        while pc < ops.len() {
            pc = self.do_op(ops.get(pc)?, pc as u32)? as usize;
        }
        Some(())
    }
    fn run_return(&mut self, ops: &[OpCode]) -> Option<()> {
        self.run_bare(ops);
        match self.stack.pop()? {
            Value::Int(b) => {
                if b == bigint::U256::zero() {
                    None
                } else {
                    Some(())
                }
            }
            _ => Some(()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum OpCode {
    // arithmetic
    ADD,
    SUB,
    MUL,
    DIV,
    REM,
    // logic
    AND,
    OR,
    XOR,
    NOT,
    EQL,
    // cryptographyy
    HASH,
    //SIGE,
    //SIGQ,
    SIGEOK,
    //SIGQOK,
    // "heap" access
    STORE,
    LOAD,
    STOREIMM(u16),
    LOADIMM(u16),
    // vector operations
    VREF,
    VAPPEND,
    VSLICE,
    VLENGTH,
    VEMPTY,
    BEMPTY,
    VPUSH,

    // control flow
    BEZ(u16),
    BNZ(u16),
    JMP(u16),
    LOOP(u16, Vec<OpCode>),
    // literals
    PUSHB(Vec<u8>),
    PUSHI(bigint::U256),
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
enum Value {
    Int(bigint::U256),
    Bytes(im_rc::Vector<u8>),
    Vector(im_rc::Vector<Value>),
}

impl Value {
    fn as_int(&self) -> Option<bigint::U256> {
        match self {
            Value::Int(bi) => Some(*bi),
            _ => None,
        }
    }
    fn as_u16(&self) -> Option<u16> {
        let num = self.as_int()?;
        if num > bigint::U256::from(65535) {
            None
        } else {
            Some(num.low_u32() as u16)
        }
    }
    fn from_bytes(bts: &[u8]) -> Self {
        let mut new = im_rc::Vector::new();
        for b in bts {
            new.push_back(*b);
        }
        Value::Bytes(new)
    }
    fn from_bool(b: bool) -> Self {
        if b {
            Value::Int(bigint::U256::one())
        } else {
            Value::Int(bigint::U256::zero())
        }
    }

    fn as_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Value::Int(bi) => {
                let mut out = vec![0; 32];
                bi.to_little_endian(&mut out);
                Some(out)
            }
            Value::Bytes(bts) => Some(bts.iter().copied().collect()),
            Value::Vector(_) => None,
        }
    }
}

impl Decodable for Value {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if rlp.is_list() {
            let vec: Vec<Value> = rlp.as_list()?;
            Ok(Value::Vector(vec.try_into().unwrap()))
        } else if rlp.is_data() {
            let vec: Vec<u8> = rlp.as_val()?;
            Ok(Value::Bytes(vec.try_into().unwrap()))
        } else if rlp.is_int() {
            let int: u64 = rlp.as_val()?;
            Ok(Value::Int(int.try_into().unwrap()))
        } else {
            Err(rlp::DecoderError::Custom("not int, list, or data"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck;
    use quickcheck_macros::*;
    fn dontcrash(data: &[u8]) {
        let script = Script(data.to_vec());
        if let Some(ops) = script.to_ops() {
            println!("{:?}", ops);
            let redone = Script::from_ops(&ops).unwrap();
            assert_eq!(redone, script);
        }
    }

    #[test]
    fn fuzz_crash_0() {
        dontcrash(&hex::decode("b000001010").unwrap())
    }

    #[test]
    fn stack_overflow() {
        let mut data = Vec::new();
        for _ in 0..100000 {
            data.push(0xb0)
        }
        dontcrash(&data.to_vec())
    }

    #[test]
    fn check_sig() {
        let (pk, sk) = tmelcrypt::ed25519_keygen();
        // (SIGEOK (LOAD 1) (PUSH pk) (VREF (VREF (LOAD 0) 6) 0))
        let check_sig_script = Script::from_ops(&[OpCode::LOOP(
            5,
            vec![
                OpCode::PUSHI(0.into()),
                OpCode::PUSHI(6.into()),
                OpCode::LOADIMM(0),
                OpCode::VREF,
                OpCode::VREF,
                OpCode::PUSHB(pk.0.to_vec()),
                OpCode::LOADIMM(1),
                OpCode::SIGEOK,
            ],
        )])
        .unwrap();
        println!("script length is {}", check_sig_script.0.len());
        let mut tx = txn::Transaction::empty_test();
        tx.sign_ed25519(sk);
        assert!(check_sig_script.check(&tx));
    }

    #[quickcheck]
    fn loop_once_is_identity(bitcode: Vec<u8>) -> bool {
        let ops = Script(bitcode.clone()).to_ops();
        let tx = txn::Transaction::empty_test();
        match ops {
            None => true,
            Some(ops) => {
                let loop_ops = vec![OpCode::LOOP(1, ops.clone())];
                let loop_script = Script::from_ops(&loop_ops).unwrap();
                let orig_script = Script::from_ops(&ops).unwrap();
                loop_script.check(&tx) == orig_script.check(&tx)
            }
        }
    }

    #[quickcheck]
    fn deterministic_execution(bitcode: Vec<u8>) -> bool {
        let ops = Script(bitcode.clone()).to_ops();
        let tx = txn::Transaction::empty_test();
        match ops {
            None => true,
            Some(ops) => {
                let orig_script = Script::from_ops(&ops).unwrap();
                let first = orig_script.check(&tx);
                let second = orig_script.check(&tx);
                first == second
            }
        }
    }
}
