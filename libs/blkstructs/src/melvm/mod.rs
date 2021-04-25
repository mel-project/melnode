pub use crate::{CoinData, CoinID, Transaction};
use crate::{CoinDataHeight, Header};
use arbitrary::Arbitrary;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryInto;
use tmelcrypt::HashVal;

mod lexer;

/// Heap address where the transaction trying to spend the coin encumbered by this covenant (spender) is put
pub const ADDR_SPENDER_TX: u16 = 0;
/// Heap address where the spender's hash is put.
pub const ADDR_SPENDER_TXHASH: u16 = 1;
/// Heap address where the *parent* (the transaction that created the coin now getting spent)'s hash is put
pub const ADDR_PARENT_TXHASH: u16 = 2;
/// Heap address where the index, at the parent, of the coin being spent is put. For example, if we are spending the third output of some transaction, `Heap[ADDR_PARENT_INDEX] = 2`.
pub const ADDR_PARENT_INDEX: u16 = 3;
/// Heap address where the hash of the running covenant is put.
pub const ADDR_SELF_HASH: u16 = 4;
/// Heap address where the face value of the coin being spent is put.
pub const ADDR_PARENT_VALUE: u16 = 5;
/// Heap address where the denomination of the coin being spent is put.
pub const ADDR_PARENT_DENOM: u16 = 6;
/// Heap address where the additional data of the coin being spent is put.
pub const ADDR_PARENT_ADDITIONAL_DATA: u16 = 7;
/// Heap address where the height of the parent is put.
pub const ADDR_PARENT_HEIGHT: u16 = 7;
/// Heap address where the "spender index" is put. For example, if this coin is spent as the first input of the spender, then `Heap[ADDR_SPENDER_INDEX] = 0`.
pub const ADDR_SPENDER_INDEX: u16 = 8;
/// Heap address where the header of the last block is put. If the covenant is being evaluated for a transaction in block N, this is the header of block N-1.
pub const ADDR_LAST_HEADER: u16 = 9;

// hm.insert(2, txhash.0.into());
// hm.insert(3, Value::Int(U256::from(*index)));

// let CoinDataHeight {
//     coin_data:
//         CoinData {
//             covhash,
//             value,
//             denom,
//             additional_data,
//         },
//     height,
// } = &env.spending_cdh;

// hm.insert(4, covhash.0.into());
// hm.insert(5, value.clone().into());
// hm.insert(6, denom.clone().into());
// hm.insert(7, additional_data.clone().into());
// hm.insert(8, height.clone().into());

#[derive(Clone, Eq, PartialEq, Debug, Arbitrary, Serialize, Deserialize, Hash)]
/// A MelVM covenant. Essentially, given a transaction that attempts to spend it, it either allows the transaction through or doesn't.
pub struct Covenant(pub Vec<u8>);

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
/// The execution environment of a covenant.
pub struct CovenantEnv<'a> {
    pub spender_coinid: &'a CoinID,
    pub spender_cdh: &'a CoinDataHeight,
    pub spender_index: u8,
    pub last_header: &'a Header,
}

impl Covenant {
    /// Checks a transaction, returning whether or not the transaction is valid.
    ///
    /// The caller must also pass in the [CoinID] and [CoinDataHeight] corresponding to the coin that's being spent, as well as the [Header] of the *previous* block (if this transaction is trying to go into block N, then the header of block N-1). This allows the covenant to access (a committment to) its execution environment, allowing constructs like timelock contracts and colored-coin-like systems.
    pub fn check(&self, tx: &Transaction, env: CovenantEnv) -> bool {
        self.check_opt(tx, env).is_some()
    }

    pub fn check_raw(&self, args: &[Value]) -> bool {
        let mut hm = HashMap::new();
        for (i, v) in args.iter().enumerate() {
            hm.insert(i as u16, v.clone());
        }
        if let Some(ops) = self.to_ops() {
            Executor::new(hm).run_return(&ops).is_some()
        } else {
            false
        }
    }

    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&self.0)
    }

    fn check_opt(&self, tx: &Transaction, env: CovenantEnv) -> Option<()> {
        let ops = self.to_ops()?;
        Executor::new_from_env(tx.clone(), env).run_return(&ops)
    }

    pub fn std_ed25519_pk(pk: tmelcrypt::Ed25519PK) -> Self {
        Covenant::from_ops(&[
            OpCode::PushI(0.into()),
            OpCode::PushI(6.into()),
            OpCode::LoadImm(0),
            OpCode::VRef,
            OpCode::VRef,
            OpCode::PushB(pk.0.to_vec()),
            OpCode::LoadImm(1),
            OpCode::SigEOk(32),
        ])
        .unwrap()
    }

    pub fn std_ed25519_pk_4(pk: tmelcrypt::Ed25519PK) -> Self {
        Covenant::from_ops(&[
            OpCode::PushI(0.into()),
            OpCode::PushI(6.into()),
            OpCode::LoadImm(0),
            OpCode::VRef,
            OpCode::VRef,
            OpCode::PushB(pk.0.to_vec()),
            OpCode::LoadImm(1),
            OpCode::SigEOk(32),
            OpCode::Bnz(8),
            OpCode::PushI(0.into()),
            OpCode::PushI(6.into()),
            OpCode::LoadImm(0),
            OpCode::VRef,
            OpCode::VRef,
            OpCode::PushB(pk.0.to_vec()),
            OpCode::LoadImm(1),
            OpCode::SigEOk(32),
        ])
        .unwrap()
    }

    pub fn from_ops(ops: &[OpCode]) -> Option<Self> {
        let mut output: Vec<u8> = Vec::new();
        // go through output
        for op in ops {
            Covenant::assemble_one(op, &mut output)?
        }
        Some(Covenant(output))
    }

    pub fn always_true() -> Self {
        Covenant::from_ops(&[OpCode::PushI(1.into())]).unwrap()
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
            0x10 => output.push(OpCode::Add),
            0x11 => output.push(OpCode::Sub),
            0x12 => output.push(OpCode::Mul),
            0x13 => output.push(OpCode::Div),
            0x14 => output.push(OpCode::Rem),
            // logic
            0x20 => output.push(OpCode::And),
            0x21 => output.push(OpCode::Or),
            0x22 => output.push(OpCode::Xor),
            0x23 => output.push(OpCode::Not),
            0x24 => output.push(OpCode::Eql),
            0x25 => output.push(OpCode::Lt),
            0x26 => output.push(OpCode::Gt),
            // cryptography
            0x30 => output.push(OpCode::Hash(u16arg(bcode)?)),
            //0x31 => output.push(OpCode::SIGE),
            0x32 => output.push(OpCode::SigEOk(u16arg(bcode)?)),
            // storage
            0x40 => output.push(OpCode::Load),
            0x41 => output.push(OpCode::Store),
            0x42 => output.push(OpCode::LoadImm(u16arg(bcode)?)),
            0x43 => output.push(OpCode::StoreImm(u16arg(bcode)?)),
            // vectors
            0x50 => output.push(OpCode::VRef),
            0x51 => output.push(OpCode::VAppend),
            0x52 => output.push(OpCode::VEmpty),
            0x53 => output.push(OpCode::VLength),
            0x54 => output.push(OpCode::BPush),
            0x55 => output.push(OpCode::VSlice),
            0x56 => output.push(OpCode::BEmpty),
            0x57 => output.push(OpCode::VSet),
            // bitwise
            0x60 => output.push(OpCode::Shl),
            0x61 => output.push(OpCode::Shr),
            0x62 => output.push(OpCode::BitAnd),
            0x63 => output.push(OpCode::BitOr),
            0x64 => output.push(OpCode::BitXor),
            // control flow
            0xa0 => output.push(OpCode::Jmp(u16arg(bcode)?)),
            0xa1 => output.push(OpCode::Bez(u16arg(bcode)?)),
            0xa2 => output.push(OpCode::Bnz(u16arg(bcode)?)),
            0xb0 => {
                let iterations = u16arg(bcode)?;
                let count = u16arg(bcode)?;
                let mut rec_output = Vec::new();
                for _ in 0..count {
                    Covenant::disassemble_one(bcode, &mut rec_output, rec_depth + 1)?;
                }
                output.push(OpCode::Loop(iterations, rec_output));
            }
            0xc0 => output.push(OpCode::ItoB),
            0xc1 => output.push(OpCode::BtoI),
            // literals
            0xf0 => {
                let strlen = bcode.pop()?;
                let mut blit = Vec::with_capacity(strlen as usize);
                for _ in 0..strlen {
                    blit.push(bcode.pop()?);
                }
                output.push(OpCode::PushB(blit))
            }
            0xf1 => {
                let mut buf = [0; 32];
                for r in buf.iter_mut() {
                    *r = bcode.pop()?
                }
                output.push(OpCode::PushI(U256::from_big_endian(&buf)))
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
            Covenant::disassemble_one(&mut reversed, &mut output, 0)?
        }
        Some(output)
    }

    pub fn weight(&self) -> Option<u128> {
        Some(self.to_ops()?.into_iter().map(|v| v.weight()).sum())
    }

    fn assemble_one(op: &OpCode, output: &mut Vec<u8>) -> Option<()> {
        match op {
            // arithmetic
            OpCode::Add => output.push(0x10),
            OpCode::Sub => output.push(0x11),
            OpCode::Mul => output.push(0x12),
            OpCode::Div => output.push(0x13),
            OpCode::Rem => output.push(0x14),
            // logic
            OpCode::And => output.push(0x20),
            OpCode::Or => output.push(0x21),
            OpCode::Xor => output.push(0x22),
            OpCode::Not => output.push(0x23),
            OpCode::Eql => output.push(0x24),
            OpCode::Lt => output.push(0x25),
            OpCode::Gt => output.push(0x26),
            // cryptography
            OpCode::Hash(n) => {
                output.push(0x30);
                output.extend(&n.to_be_bytes());
            }
            //OpCode::SIGE => output.push(0x31),
            OpCode::SigEOk(n) => {
                output.push(0x32);
                output.extend(&n.to_be_bytes())
            }
            // storage
            OpCode::Load => output.push(0x40),
            OpCode::Store => output.push(0x41),
            OpCode::LoadImm(idx) => {
                output.push(0x42);
                output.extend(&idx.to_be_bytes());
            }
            OpCode::StoreImm(idx) => {
                output.push(0x43);
                output.extend(&idx.to_be_bytes());
            }
            // vectors
            OpCode::VRef => output.push(0x50),
            OpCode::VAppend => output.push(0x51),
            OpCode::VEmpty => output.push(0x52),
            OpCode::VLength => output.push(0x53),
            OpCode::BPush => output.push(0x54),
            OpCode::VSlice => output.push(0x55),
            OpCode::BEmpty => output.push(0x56),
            OpCode::VSet => output.push(0x57),
            // bitwise
            OpCode::Shl => output.push(0x60),
            OpCode::Shr => output.push(0x61),
            OpCode::BitAnd => output.push(0x62),
            OpCode::BitOr => output.push(0x63),
            OpCode::BitXor => output.push(0x64),
            // control flow
            OpCode::Jmp(val) => {
                output.push(0xa0);
                output.extend_from_slice(&val.to_be_bytes());
            }
            OpCode::Bez(val) => {
                output.push(0xa1);
                output.extend_from_slice(&val.to_be_bytes());
            }
            OpCode::Bnz(val) => {
                output.push(0xa2);
                output.extend_from_slice(&val.to_be_bytes());
            }
            OpCode::Loop(iterations, ops) => {
                output.push(0xb0);
                output.extend_from_slice(&iterations.to_be_bytes());
                let op_cnt: u16 = ops.len().try_into().ok()?;
                output.extend_from_slice(&op_cnt.to_be_bytes());
                for op in ops {
                    Covenant::assemble_one(op, output)?
                }
            }
            // type conversions
            OpCode::ItoB => output.push(0xc0),
            OpCode::BtoI => output.push(0xc1),

            // literals
            OpCode::PushB(bts) => {
                output.push(0xf0);
                if bts.len() > 255 {
                    return None;
                }
                output.push(bts.len() as u8);
                output.extend_from_slice(bts);
            }
            OpCode::PushI(num) => {
                output.push(0xf1);
                let mut out = [0; 32];
                num.to_big_endian(&mut out);
                output.extend_from_slice(&out);
            }
        }
        Some(())
    }
}

pub struct Executor {
    pub stack: Vec<Value>,
    pub heap: HashMap<u16, Value>,
}

impl Executor {
    pub fn new(heap_init: HashMap<u16, Value>) -> Self {
        Executor {
            stack: Vec::new(),
            heap: heap_init,
        }
    }
    pub fn new_from_env(tx: Transaction, env: CovenantEnv) -> Self {
        let mut hm = HashMap::new();
        hm.insert(ADDR_SPENDER_TXHASH, Value::from_bytes(&tx.hash_nosigs().0));
        let tx_val = Value::from(tx);
        hm.insert(ADDR_SPENDER_TX, tx_val);

        let CoinID { txhash, index } = &env.spender_coinid;

        hm.insert(ADDR_PARENT_TXHASH, txhash.0.into());
        hm.insert(ADDR_PARENT_INDEX, Value::Int(U256::from(*index)));

        let CoinDataHeight {
            coin_data:
                CoinData {
                    covhash,
                    value,
                    denom,
                    additional_data,
                },
            height,
        } = &env.spender_cdh;

        hm.insert(ADDR_SELF_HASH, covhash.0.into());
        hm.insert(ADDR_PARENT_VALUE, value.clone().into());
        hm.insert(ADDR_PARENT_DENOM, denom.clone().into());
        hm.insert(ADDR_PARENT_ADDITIONAL_DATA, additional_data.clone().into());
        hm.insert(ADDR_PARENT_HEIGHT, height.clone().into());
        hm.insert(ADDR_LAST_HEADER, Value::from(*env.last_header));
        hm.insert(ADDR_SPENDER_INDEX, Value::from(env.spender_index as u64));

        Executor::new(hm)
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
    pub fn do_op(&mut self, op: &OpCode, pc: u32) -> Option<u32> {
        match op {
            // arithmetic
            OpCode::Add => {
                self.do_binop(|x, y| Some(Value::Int(x.as_int()?.overflowing_add(y.as_int()?).0)))?
            }
            OpCode::Sub => {
                self.do_binop(|x, y| Some(Value::Int(x.as_int()?.overflowing_sub(y.as_int()?).0)))?
            }
            OpCode::Mul => {
                self.do_binop(|x, y| Some(Value::Int(x.as_int()?.overflowing_mul(y.as_int()?).0)))?
            }
            OpCode::Div => self.do_binop(|x, y| {
                if y.as_int()? == U256::zero() {
                    None
                } else {
                    Some(Value::Int(
                        x.as_int()?
                            .checked_add(y.as_int()?)
                            .unwrap_or_else(|| 0.into()),
                    ))
                }
            })?,
            OpCode::Rem => self.do_binop(|x, y| {
                if y.as_int()? == U256::zero() {
                    None
                } else {
                    Some(Value::Int(
                        x.as_int()?
                            .checked_rem(y.as_int()?)
                            .unwrap_or_else(|| 0.into()),
                    ))
                }
            })?,
            // logic
            OpCode::And => self.do_binop(|x, y| Some(Value::Int(x.as_int()? & y.as_int()?)))?,
            OpCode::Or => self.do_binop(|x, y| Some(Value::Int(x.as_int()? | y.as_int()?)))?,
            OpCode::Xor => self.do_binop(|x, y| Some(Value::Int(x.as_int()? ^ y.as_int()?)))?,
            OpCode::Not => self.do_monop(|x| Some(Value::Int(!x.as_int()?)))?,
            OpCode::Eql => self.do_binop(|x, y| match (x, y) {
                (Value::Int(x), Value::Int(y)) => {
                    if x == y {
                        Some(Value::Int(U256::one()))
                    } else {
                        Some(Value::Int(U256::zero()))
                    }
                }
                (Value::Bytes(x), Value::Bytes(y)) => {
                    if x.len() == y.len() && x.iter().zip(y).all(|(a, ref b)| a == b) {
                        Some(Value::Int(U256::one()))
                    } else {
                        Some(Value::Int(U256::zero()))
                    }
                }
                _ => None,
            })?,
            OpCode::Lt => self.do_binop(|x, y| {
                let x = x.as_int()?;
                let y = y.as_int()?;
                if x.overflowing_sub(y).1 {
                    Some(Value::Int(U256::one()))
                } else {
                    Some(Value::Int(U256::zero()))
                }
            })?,
            OpCode::Gt => self.do_binop(|x, y| {
                let x = x.as_int()?;
                let y = y.as_int()?;
                if !x.overflowing_sub(y).1 {
                    Some(Value::Int(U256::one()))
                } else {
                    Some(Value::Int(U256::zero()))
                }
            })?,
            OpCode::Shl => self.do_binop(|x, offset| {
                let x = x.as_int()?;
                let offset = offset.as_int()?;
                Some(Value::Int(x << offset))
            })?,
            OpCode::Shr => self.do_binop(|x, offset| {
                let x = x.as_int()?;
                let offset = offset.as_int()?;
                Some(Value::Int(x >> offset))
            })?,
            OpCode::BitAnd => self.do_binop(|x, y| {
                let x = x.as_int()?;
                let y = y.as_int()?;
                Some(Value::Int(x & y))
            })?,
            OpCode::BitOr => self.do_binop(|x, y| {
                let x = x.as_int()?;
                let y = y.as_int()?;
                Some(Value::Int(x | y))
            })?,
            OpCode::BitXor => self.do_binop(|x, y| {
                let x = x.as_int()?;
                let y = y.as_int()?;
                Some(Value::Int(x ^ y))
            })?,
            // cryptography
            OpCode::Hash(n) => self.do_monop(|to_hash| {
                let to_hash = to_hash.as_bytes()?;
                if to_hash.len() > *n as usize {
                    return None;
                }
                let hash = tmelcrypt::hash_single(&to_hash.iter().cloned().collect::<Vec<_>>());
                Some(Value::from_bytes(&hash.0))
            })?,
            OpCode::SigEOk(n) => self.do_triop(|message, public_key, signature| {
                //println!("SIGEOK({:?}, {:?}, {:?})", message, public_key, signature);
                let pk = public_key.as_bytes()?;
                if pk.len() > 32 {
                    return Some(Value::from_bool(false));
                }
                let pk_b: Vec<u8> = pk.iter().cloned().collect();
                let public_key = tmelcrypt::Ed25519PK::from_bytes(&pk_b)?;
                let message = message.as_bytes()?;
                if message.len() > *n as usize {
                    return None;
                }
                let message: Vec<u8> = message.iter().cloned().collect();
                let signature = signature.as_bytes()?;
                if signature.len() > 64 {
                    return Some(Value::from_bool(false));
                }
                let signature: Vec<u8> = signature.iter().cloned().collect();
                Some(Value::from_bool(public_key.verify(&message, &signature)))
            })?,
            // storage access
            OpCode::Store => {
                let addr = self.stack.pop()?.as_u16()?;
                let val = self.stack.pop()?;
                self.heap.insert(addr, val);
            }
            OpCode::Load => {
                let addr = self.stack.pop()?.as_u16()?;
                let res = self.heap.get(&addr)?.clone();
                self.stack.push(res)
            }
            OpCode::StoreImm(idx) => {
                let val = self.stack.pop()?;
                self.heap.insert(*idx, val);
            }
            OpCode::LoadImm(idx) => {
                let res = self.heap.get(idx)?.clone();
                self.stack.push(res)
            }
            // vector operations
            OpCode::VRef => self.do_binop(|vec, idx| {
                let idx = idx.as_u16()? as usize;
                match vec {
                    Value::Bytes(bts) => Some(Value::Int(U256::from(*bts.get(idx)?))),
                    Value::Vector(elems) => Some(elems.get(idx)?.clone()),
                    _ => None,
                }
            })?,
            OpCode::VSet => self.do_triop(|vec, idx, value| {
                let idx = idx.as_u16()? as usize;
                match vec {
                    Value::Bytes(mut bts) => {
                        let converted = value.as_u16()? as u8;
                        bts[idx] = converted;
                        Some(Value::Bytes(bts))
                    }
                    Value::Vector(mut elems) => {
                        elems[idx] = value;
                        Some(Value::Vector(elems))
                    }
                    _ => None,
                }
            })?,
            OpCode::VAppend => self.do_binop(|v1, v2| match (v1, v2) {
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
            OpCode::VSlice => self.do_triop(|vec, i, j| {
                let i = i.as_u16()? as usize;
                let j = j.as_u16()? as usize;
                match vec {
                    Value::Vector(mut vec) => Some(Value::Vector(vec.slice(i..j))),
                    Value::Bytes(mut vec) => Some(Value::Bytes(vec.slice(i..j))),
                    _ => None,
                }
            })?,
            OpCode::VLength => self.do_monop(|vec| match vec {
                Value::Vector(vec) => Some(Value::Int(U256::from(vec.len()))),
                Value::Bytes(vec) => Some(Value::Int(U256::from(vec.len()))),
                _ => None,
            })?,
            OpCode::VEmpty => self.stack.push(Value::Vector(im::Vector::new())),
            OpCode::BEmpty => self.stack.push(Value::Bytes(im::Vector::new())),
            OpCode::BPush => self.do_binop(|vec, val| match vec {
                Value::Vector(mut vec) => {
                    vec.push_back(val);
                    Some(Value::Vector(vec))
                }
                Value::Bytes(mut vec) => {
                    let bts = val.as_int()?;
                    if bts > U256::from(255) {
                        return None;
                    }
                    let bts = bts.low_u32() as u8;
                    vec.push_back(bts);
                    Some(Value::Bytes(vec))
                }
                _ => None,
            })?,
            // control flow
            OpCode::Bez(jgap) => {
                let top = self.stack.pop()?;
                if top == Value::Int(U256::zero()) {
                    return Some(pc + 1 + *jgap as u32);
                }
            }
            OpCode::Bnz(jgap) => {
                let top = self.stack.pop()?;
                if top != Value::Int(U256::zero()) {
                    return Some(pc + 1 + *jgap as u32);
                }
            }
            OpCode::Jmp(jgap) => return Some(pc + 1 + *jgap as u32),
            OpCode::Loop(iterations, ops) => {
                for _ in 0..*iterations {
                    self.run_bare(&ops)?
                }
            }
            // Conversions
            OpCode::BtoI => self.do_monop(|x| {
                let mut bytes = x.as_bytes()?;
                if bytes.len() < 32 {
                    return None;
                }

                Some(Value::Int(
                    bytes
                        .slice(..32)
                        .iter()
                        .fold(U256::zero(), |acc, b| (acc * 256) + *b),
                ))
            })?,
            OpCode::ItoB => self.do_monop(|x| {
                let n = x.as_int()?;
                let mut bytes = im::vector![];
                for i in 0..32 {
                    bytes.push_back(n.byte(i));
                }
                Some(Value::Bytes(bytes))
            })?,
            // literals
            OpCode::PushB(bts) => {
                let bts = Value::from_bytes(bts);
                self.stack.push(bts);
            }
            OpCode::PushI(num) => self.stack.push(Value::Int(*num)),
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
                if b == U256::zero() {
                    None
                } else {
                    Some(())
                }
            }
            _ => Some(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpCode {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // logic
    And,
    Or,
    Xor,
    Not,
    Eql,
    Lt,
    Gt,
    // bitwise
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    // cryptographyy
    Hash(u16),
    //SIGE,
    //SIGQ,
    SigEOk(u16),
    //SIGQOK,
    // "heap" access
    Store,
    Load,
    StoreImm(u16),
    LoadImm(u16),
    // vector operations
    VRef,
    VSet,
    VAppend,
    VSlice,
    VLength,
    VEmpty,
    BEmpty,
    BPush,

    // control flow
    Bez(u16),
    Bnz(u16),
    Jmp(u16),
    Loop(u16, Vec<OpCode>),

    // type conversions
    ItoB,
    BtoI,
    // SERIAL(u16),

    // literals
    PushB(Vec<u8>),
    PushI(U256),
}

impl OpCode {
    pub fn weight(&self) -> u128 {
        match self {
            OpCode::Add => 4,
            OpCode::Sub => 4,
            OpCode::Mul => 6,
            OpCode::Div => 6,
            OpCode::Rem => 6,

            OpCode::And => 4,
            OpCode::Or => 4,
            OpCode::Xor => 4,
            OpCode::Not => 4,
            OpCode::Eql => 4,
            OpCode::Lt => 4,
            OpCode::Gt => 4,

            OpCode::Shl => 4,
            OpCode::Shr => 4,
            OpCode::BitAnd => 4,
            OpCode::BitOr => 4,
            OpCode::BitXor => 4,

            OpCode::Hash(n) => 50 + *n as u128,
            OpCode::SigEOk(n) => 100 + *n as u128,

            OpCode::Store => 10,
            OpCode::Load => 10,
            OpCode::StoreImm(_) => 4,
            OpCode::LoadImm(_) => 4,

            OpCode::VRef => 10,
            OpCode::VSet => 50,
            OpCode::VAppend => 50,
            OpCode::VSlice => 50,
            OpCode::VLength => 10,
            OpCode::VEmpty => 4,
            OpCode::BEmpty => 4,
            OpCode::BPush => 10,

            OpCode::ItoB => 50,
            OpCode::BtoI => 50,

            OpCode::Bez(_) => 1,
            OpCode::Bnz(_) => 1,
            OpCode::Jmp(_) => 1,
            OpCode::Loop(loops, contents) => {
                let one_iteration: u128 = contents.iter().map(|o| o.weight()).sum();
                one_iteration.saturating_mul(*loops as _)
            }

            OpCode::PushB(_) => 1,
            OpCode::PushI(_) => 1,
        }
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum Value {
    Int(U256),
    Bytes(im::Vector<u8>),
    Vector(im::Vector<Value>),
}

impl Value {
    fn as_int(&self) -> Option<U256> {
        match self {
            Value::Int(bi) => Some(*bi),
            _ => None,
        }
    }
    fn as_u16(&self) -> Option<u16> {
        let num = self.as_int()?;
        if num > U256::from(65535) {
            None
        } else {
            Some(num.low_u32() as u16)
        }
    }
    pub fn from_bytes(bts: &[u8]) -> Self {
        let mut new = im::Vector::new();
        for b in bts {
            new.push_back(*b);
        }
        Value::Bytes(new)
    }
    fn from_bool(b: bool) -> Self {
        if b {
            Value::Int(U256::one())
        } else {
            Value::Int(U256::zero())
        }
    }

    fn as_bytes(&self) -> Option<im::Vector<u8>> {
        match self {
            Value::Bytes(bts) => Some(bts.clone()),
            _ => None,
        }
    }
}

impl From<u128> for Value {
    fn from(n: u128) -> Self {
        Value::Int(U256::from(n))
    }
}

impl From<u64> for Value {
    fn from(n: u64) -> Self {
        Value::Int(U256::from(n))
    }
}

impl From<CoinData> for Value {
    fn from(cd: CoinData) -> Self {
        Value::Vector(im::vector![
            cd.covhash.0.into(),
            cd.value.into(),
            cd.denom.into(),
            cd.additional_data.into()
        ])
    }
}

impl From<Header> for Value {
    fn from(cd: Header) -> Self {
        Value::Vector(im::vector![
            (cd.network as u64).into(),
            cd.previous.into(),
            cd.height.into(),
            cd.history_hash.into(),
            cd.coins_hash.into(),
            cd.transactions_hash.into(),
            cd.fee_pool.into(),
            cd.fee_multiplier.into(),
            cd.dosc_speed.into(),
            cd.pools_hash.into(),
            cd.stakes_hash.into()
        ])
    }
}

impl From<CoinDataHeight> for Value {
    fn from(cd: CoinDataHeight) -> Self {
        Value::Vector(im::vector![cd.coin_data.into(), cd.height.into()])
    }
}

impl From<CoinID> for Value {
    fn from(c: CoinID) -> Self {
        Value::Vector(im::vector![
            c.txhash.0.into(),
            Value::Int(U256::from(c.index))
        ])
    }
}

impl From<Covenant> for Value {
    fn from(c: Covenant) -> Self {
        Value::Bytes(c.0.into())
    }
}

impl From<[u8; 32]> for Value {
    fn from(v: [u8; 32]) -> Self {
        Value::Bytes(v.iter().cloned().collect::<im::Vector<u8>>())
    }
}

impl From<HashVal> for Value {
    fn from(v: HashVal) -> Self {
        Value::Bytes(v.iter().cloned().collect::<im::Vector<u8>>())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v.into_iter().collect::<im::Vector<u8>>())
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::Vector(
            v.into_iter()
                .map(|x| x.into())
                .collect::<im::Vector<Value>>(),
        )
    }
}

impl From<Transaction> for Value {
    fn from(tx: Transaction) -> Self {
        Value::Vector(im::vector![
            Value::Int(U256::from(tx.kind as u8)),
            tx.inputs.into(),
            tx.outputs.into(),
            tx.fee.into(),
            tx.scripts.into(),
            tx.data.into(),
            tx.sigs.into()
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck;
    use quickcheck_macros::*;
    fn dontcrash(data: &[u8]) {
        let script = Covenant(data.to_vec());
        if let Some(ops) = script.to_ops() {
            println!("{:?}", ops);
            let redone = Covenant::from_ops(&ops).unwrap();
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
        let check_sig_script = Covenant::from_ops(&[OpCode::Loop(
            5,
            vec![
                OpCode::PushI(0.into()),
                OpCode::PushI(6.into()),
                OpCode::LoadImm(0),
                OpCode::VRef,
                OpCode::VRef,
                OpCode::PushB(pk.0.to_vec()),
                OpCode::LoadImm(1),
                OpCode::SigEOk(32),
            ],
        )])
        .unwrap();
        println!("script length is {}", check_sig_script.0.len());
        let mut tx = Transaction::empty_test().signed_ed25519(sk);
        assert!(check_sig_script.check(&tx, &[]));
        tx.sigs[0][0] ^= 123;
        assert!(!check_sig_script.check(&tx, &[]));
    }

    #[quickcheck]
    fn loop_once_is_identity(bitcode: Vec<u8>) -> bool {
        let ops = Covenant(bitcode.clone()).to_ops();
        let tx = Transaction::empty_test();
        match ops {
            None => true,
            Some(ops) => {
                let loop_ops = vec![OpCode::Loop(1, ops.clone())];
                let loop_script = Covenant::from_ops(&loop_ops).unwrap();
                let orig_script = Covenant::from_ops(&ops).unwrap();
                loop_script.check(&tx, &[]) == orig_script.check(&tx, &[])
            }
        }
    }

    #[quickcheck]
    fn deterministic_execution(bitcode: Vec<u8>) -> bool {
        let ops = Covenant(bitcode.clone()).to_ops();
        let tx = Transaction::empty_test();
        match ops {
            None => true,
            Some(ops) => {
                let orig_script = Covenant::from_ops(&ops).unwrap();
                let first = orig_script.check(&tx, &[]);
                let second = orig_script.check(&tx, &[]);
                first == second
            }
        }
    }
}
