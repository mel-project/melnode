pub use crate::{CoinData, CoinID, Transaction};
use crate::{CoinDataHeight, Denom, Header, HexBytes};
use arbitrary::Arbitrary;
use ethnum::U256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryInto;
use tmelcrypt::HashVal;

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
pub const ADDR_PARENT_HEIGHT: u16 = 8;
/// Heap address where the "spender index" is put. For example, if this coin is spent as the first input of the spender, then `Heap[ADDR_SPENDER_INDEX] = 0`.
pub const ADDR_SPENDER_INDEX: u16 = 9;
/// Heap address where the header of the last block is put. If the covenant is being evaluated for a transaction in block N, this is the header of block N-1.
pub const ADDR_LAST_HEADER: u16 = 10;

#[derive(Clone, Eq, PartialEq, Debug, Arbitrary, Serialize, Deserialize, Hash)]
/// A MelVM covenant. Essentially, given a transaction that attempts to spend it, it either allows the transaction through or doesn't.
pub struct Covenant(#[serde(with = "stdcode::hex")] pub Vec<u8>);

/// A pointer to the currently executing instruction.
type ProgramCounter = usize;

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
/// The execution environment of a covenant.
pub struct CovenantEnv<'a> {
    pub parent_coinid: &'a CoinID,
    pub parent_cdh: &'a CoinDataHeight,
    pub spender_index: u8,
    pub last_header: &'a Header,
}

impl Covenant {
    /// Checks a transaction, returning whether or not the transaction is valid.
    ///
    /// The caller must also pass in the [CoinID] and [CoinDataHeight] corresponding to the coin that's being spent, as well as the [Header] of the *previous* block (if this transaction is trying to go into block N, then the header of block N-1). This allows the covenant to access (a committment to) its execution environment, allowing constructs like timelock contracts and colored-coin-like systems.
    pub fn check(&self, tx: &Transaction, env: CovenantEnv) -> bool {
        self.check_opt_env(tx, Some(env))
    }

    pub(crate) fn check_no_env(&self, tx: &Transaction) -> bool {
        self.check_opt_env(tx, None)
    }

    /// Execute a transaction in a [CovenantEnv] to completion and return the
    fn check_opt_env(&self, tx: &Transaction, env: Option<CovenantEnv>) -> bool {
        if let Some(ops) = self.to_ops() {
            Executor::new_from_env(tx.clone(), env).run_return(&ops)
        } else {
            false
        }
    }

    pub fn check_raw(&self, args: &[Value]) -> bool {
        let mut hm = HashMap::new();
        for (i, v) in args.iter().enumerate() {
            hm.insert(i as u16, v.clone());
        }
        if let Some(ops) = self.to_ops() {
            Executor::new(hm).run_return(&ops)
        } else {
            false
        }
    }

    pub fn hash(&self) -> tmelcrypt::HashVal {
        tmelcrypt::hash_single(&self.0)
    }

    /// Returns a legacy ed25519 signature checking covenant, which checks the *first* signature.
    pub fn std_ed25519_pk_legacy(pk: tmelcrypt::Ed25519PK) -> Self {
        Covenant::from_ops(&[
            OpCode::PushI(0u32.into()),
            OpCode::PushI(6u32.into()),
            OpCode::LoadImm(ADDR_SPENDER_TX),
            OpCode::VRef,
            OpCode::VRef,
            OpCode::PushB(pk.0.to_vec()),
            OpCode::LoadImm(1),
            OpCode::SigEOk(32),
        ])
        .unwrap()
    }

    /// Returns a new ed25519 signature checking covenant, which checks the *nth* signature when spent as the nth input.
    pub fn std_ed25519_pk_new(pk: tmelcrypt::Ed25519PK) -> Self {
        Covenant::from_ops(&[
            OpCode::LoadImm(ADDR_SPENDER_INDEX),
            OpCode::PushI(6u32.into()),
            OpCode::LoadImm(ADDR_SPENDER_TX),
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
        Covenant::from_ops(&[OpCode::PushI(1u32.into())]).unwrap()
    }

    fn disassemble_one(bcode: &mut Vec<u8>, output: &mut Vec<OpCode>) -> Option<()> {
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
            0x27 => output.push(OpCode::Shl),
            0x28 => output.push(OpCode::Shr),
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
            0x54 => output.push(OpCode::VSlice),
            0x55 => output.push(OpCode::VSet),
            0x56 => output.push(OpCode::VPush),
            0x57 => output.push(OpCode::VCons),
            // bytes
            0x70 => output.push(OpCode::BRef),
            0x71 => output.push(OpCode::BAppend),
            0x72 => output.push(OpCode::BEmpty),
            0x73 => output.push(OpCode::BLength),
            0x74 => output.push(OpCode::BSlice),
            0x75 => output.push(OpCode::BSet),
            0x76 => output.push(OpCode::BPush),
            0x77 => output.push(OpCode::BCons),
            // control flow
            0xa0 => output.push(OpCode::Jmp(u16arg(bcode)?)),
            0xa1 => output.push(OpCode::Bez(u16arg(bcode)?)),
            0xa2 => output.push(OpCode::Bnz(u16arg(bcode)?)),
            0xb0 => {
                let iterations = u16arg(bcode)?;
                let count = u16arg(bcode)?;
                output.push(OpCode::Loop(iterations, count));
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
                output.push(OpCode::PushI(U256::from_be_bytes(buf)))
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
            Covenant::disassemble_one(&mut reversed, &mut output)?
        }
        Some(output)
    }

    pub fn weight(&self) -> Option<u128> {
        let ops = self.to_ops()?;
        Some(opcodes_weight(&ops))
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
            OpCode::Shl => output.push(0x27),
            OpCode::Shr => output.push(0x28),
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
            OpCode::VSlice => output.push(0x54),
            OpCode::VSet => output.push(0x55),
            OpCode::VPush => output.push(0x56),
            OpCode::VCons => output.push(0x57),
            // bytes
            OpCode::BRef => output.push(0x70),
            OpCode::BAppend => output.push(0x71),
            OpCode::BEmpty => output.push(0x72),
            OpCode::BLength => output.push(0x73),
            OpCode::BSlice => output.push(0x74),
            OpCode::BSet => output.push(0x75),
            OpCode::BPush => output.push(0x76),
            OpCode::BCons => output.push(0x77),
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
            OpCode::Loop(iterations, op_count) => {
                output.push(0xb0);
                output.extend_from_slice(&iterations.to_be_bytes());
                output.extend_from_slice(&op_count.to_be_bytes());
            }
            // type conversions
            OpCode::ItoB => output.push(0xc0),
            OpCode::BtoI => output.push(0xc1),

            OpCode::TypeQ => output.push(0xcf),

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
                let out = num.to_be_bytes();
                output.extend_from_slice(&out);
            }

            OpCode::Dup => {
                output.push(0xff);
            }
        }
        Some(())
    }
}

/// Internal tracking of state during a loop in [Executor].
struct LoopState {
    /// Pointer to first op in loop
    begin: ProgramCounter,
    /// Pointer to last op in loop (inclusive)
    end: ProgramCounter,
    /// Total number of iterations
    iterations_left: u16,
}

pub struct Executor {
    pub stack: Vec<Value>,
    pub heap: HashMap<u16, Value>,
    /// Program counter
    pc: ProgramCounter,
    /// Marks the (begin, end) of the loop if currently in one
    loop_state: Vec<LoopState>,
}

impl Executor {
    pub fn new(heap_init: HashMap<u16, Value>) -> Self {
        Executor {
            stack: Vec::new(),
            heap: heap_init,
            pc: 0,
            loop_state: vec![],
        }
    }
    pub fn new_from_env(tx: Transaction, env: Option<CovenantEnv>) -> Self {
        let mut hm = HashMap::new();
        hm.insert(ADDR_SPENDER_TXHASH, Value::from_bytes(&tx.hash_nosigs().0));
        let tx_val = Value::from(tx);
        hm.insert(ADDR_SPENDER_TX, tx_val);
        if let Some(env) = env {
            let CoinID { txhash, index } = &env.parent_coinid;

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
            } = &env.parent_cdh;

            hm.insert(ADDR_SELF_HASH, covhash.0.into());
            hm.insert(ADDR_PARENT_VALUE, value.clone().into());
            hm.insert(ADDR_PARENT_DENOM, denom.clone().into());
            hm.insert(ADDR_PARENT_ADDITIONAL_DATA, additional_data.clone().into());
            hm.insert(ADDR_PARENT_HEIGHT, height.clone().into());
            hm.insert(ADDR_LAST_HEADER, Value::from(*env.last_header));
            hm.insert(ADDR_SPENDER_INDEX, Value::from(env.spender_index as u64));
        }

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
    pub fn pc(&self) -> ProgramCounter {
        self.pc
    }
    /// Execute one instruction and update internal VM state
    pub fn step(&mut self, op: &OpCode) -> Option<()> {
        log::trace!("pc = {}", self.pc);
        if let Some(pc_diff) = self.do_op(op) {
            self.update_pc_state(pc_diff);
            Some(())
        } else {
            None
        }
    }
    /// Update program pointer state (to be called after a step)
    fn update_pc_state(&mut self, pc_diff: ProgramCounter) {
        // Update program counter
        self.pc += pc_diff;

        if let Some(mut state) = self.loop_state.pop() {
            // If done with body of loop
            if self.pc > state.end {
                // But not finished with all iterations, and did not jump outside the loop
                if state.iterations_left > 0 && self.pc.saturating_sub(state.end) == 1 {
                    log::trace!("{} iterations left", state.iterations_left);
                    // loop again
                    state.iterations_left -= 1;
                    self.pc = state.begin;
                    self.loop_state.push(state);
                }
                // If finished with all iterations, check for another loop state
                else {
                    self.update_pc_state(0);
                }
            } else {
                // If not done with loop body, resume
                self.loop_state.push(state);
            }
        }
        /*
        if let Some(ref mut state) = self.loop_state {
        //if !self.loop_state.is_empty() {
            if self.pc > state.end {
                if state.cur_iteration >= state.iterations-1 {
                    // continue past the loop
                    self.loop_state = None;
                } else {
                    // loop again
                    state.cur_iteration += 1;
                    self.pc = state.begin;
                }
            }
        }
        */
    }
    /// Execute an instruction, modifying state and return number of instructions to move forward
    pub fn do_op(&mut self, op: &OpCode) -> Option<ProgramCounter> {
        log::trace!("do_op {:?}", op);
        match op {
            // arithmetic
            OpCode::Add => self.do_binop(|x, y| {
                Some(Value::Int(x.into_int()?.overflowing_add(y.into_int()?).0))
            })?,
            OpCode::Sub => self.do_binop(|x, y| {
                Some(Value::Int(x.into_int()?.overflowing_sub(y.into_int()?).0))
            })?,
            OpCode::Mul => self.do_binop(|x, y| {
                Some(Value::Int(x.into_int()?.overflowing_mul(y.into_int()?).0))
            })?,
            OpCode::Div => {
                self.do_binop(|x, y| Some(Value::Int(x.into_int()?.checked_div(y.into_int()?)?)))?
            }
            OpCode::Rem => {
                self.do_binop(|x, y| Some(Value::Int(x.into_int()?.checked_rem(y.into_int()?)?)))?
            }
            // logic
            OpCode::And => self.do_binop(|x, y| Some(Value::Int(x.into_int()? & y.into_int()?)))?,
            OpCode::Or => self.do_binop(|x, y| Some(Value::Int(x.into_int()? | y.into_int()?)))?,
            OpCode::Xor => self.do_binop(|x, y| Some(Value::Int(x.into_int()? ^ y.into_int()?)))?,
            OpCode::Not => self.do_monop(|x| Some(Value::Int(!x.into_int()?)))?,
            OpCode::Eql => self.do_binop(|x, y| match (x, y) {
                (Value::Int(x), Value::Int(y)) => {
                    if x == y {
                        Some(Value::Int(1u32.into()))
                    } else {
                        Some(Value::Int(0u32.into()))
                    }
                }
                _ => None,
            })?,
            OpCode::Lt => self.do_binop(|x, y| {
                let x = x.into_int()?;
                let y = y.into_int()?;
                if x < y {
                    Some(Value::Int(1u32.into()))
                } else {
                    Some(Value::Int(0u32.into()))
                }
            })?,
            OpCode::Gt => self.do_binop(|x, y| {
                let x = x.into_int()?;
                let y = y.into_int()?;
                if !x > y {
                    Some(Value::Int(1u32.into()))
                } else {
                    Some(Value::Int(0u32.into()))
                }
            })?,
            OpCode::Shl => self.do_binop(|x, offset| {
                let x = x.into_int()?;
                let offset = offset.into_int()?;
                Some(Value::Int(x.wrapping_shl(offset.as_u32())))
            })?,
            OpCode::Shr => self.do_binop(|x, offset| {
                let x = x.into_int()?;
                let offset = offset.into_int()?;
                Some(Value::Int(x.wrapping_shr(offset.as_u32())))
            })?,
            // cryptography
            OpCode::Hash(n) => self.do_monop(|to_hash| {
                let to_hash = to_hash.into_bytes()?;
                if to_hash.len() > *n as usize {
                    return None;
                }
                let hash = tmelcrypt::hash_single(&to_hash.iter().cloned().collect::<Vec<_>>());
                Some(Value::from_bytes(&hash.0))
            })?,
            OpCode::SigEOk(n) => self.do_triop(|message, public_key, signature| {
                //println!("SIGEOK({:?}, {:?}, {:?})", message, public_key, signature);
                let pk = public_key.into_bytes()?;
                if pk.len() > 32 {
                    return Some(Value::from_bool(false));
                }
                let pk_b: Vec<u8> = pk.iter().cloned().collect();
                let public_key = tmelcrypt::Ed25519PK::from_bytes(&pk_b)?;
                let message = message.into_bytes()?;
                if message.len() > *n as usize {
                    return None;
                }
                let message: Vec<u8> = message.iter().cloned().collect();
                let signature = signature.into_bytes()?;
                if signature.len() > 64 {
                    return Some(Value::from_bool(false));
                }
                let signature: Vec<u8> = signature.iter().cloned().collect();
                Some(Value::from_bool(public_key.verify(&message, &signature)))
            })?,
            // storage access
            OpCode::Store => {
                let addr = self.stack.pop()?.into_u16()?;
                let val = self.stack.pop()?;
                self.heap.insert(addr, val);
            }
            OpCode::Load => {
                let addr = self.stack.pop()?.into_u16()?;
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
                let idx = idx.into_u16()? as usize;
                Some(vec.into_vector()?.get(idx)?.clone())
            })?,
            OpCode::VSet => self.do_triop(|vec, idx, value| {
                let idx = idx.into_u16()? as usize;
                let mut vec = vec.into_vector()?;
                if idx < vec.len() {
                    vec.set(idx, value);
                    Some(Value::Vector(vec))
                } else {
                    None
                }
            })?,
            OpCode::VAppend => self.do_binop(|v1, v2| {
                let mut v1 = v1.into_vector()?;
                let v2 = v2.into_vector()?;
                v1.append(v2);
                Some(Value::Vector(v1))
            })?,
            OpCode::VSlice => self.do_triop(|vec, i, j| {
                let i = i.into_u16()? as usize;
                let j = j.into_u16()? as usize;
                match vec {
                    Value::Vector(mut vec) => {
                        if j >= vec.len() || j <= i {
                            Some(Value::Vector(im::Vector::new()))
                        } else {
                            Some(Value::Vector(vec.slice(i..j)))
                        }
                    }
                    _ => None,
                }
            })?,
            OpCode::VLength => self.do_monop(|vec| match vec {
                Value::Vector(vec) => Some(Value::Int(U256::from(vec.len() as u64))),
                _ => None,
            })?,
            OpCode::VEmpty => self.stack.push(Value::Vector(im::Vector::new())),
            OpCode::VPush => self.do_binop(|vec, item| {
                let mut vec = vec.into_vector()?;
                vec.push_back(item);
                Some(Value::Vector(vec))
            })?,
            OpCode::VCons => self.do_binop(|item, vec| {
                let mut vec = vec.into_vector()?;
                vec.push_front(item);
                Some(Value::Vector(vec))
            })?,
            // bit stuff
            OpCode::BEmpty => self.stack.push(Value::Bytes(im::Vector::new())),
            OpCode::BPush => self.do_binop(|vec, val| {
                let mut vec = vec.into_bytes()?;
                let val = val.into_int()?;
                vec.push_back(*val.low() as u8);
                Some(Value::Bytes(vec))
            })?,
            OpCode::BCons => self.do_binop(|item, vec| {
                let mut vec = vec.into_bytes()?;
                vec.push_front(item.into_truncated_u8()?);
                Some(Value::Bytes(vec))
            })?,
            OpCode::BRef => self.do_binop(|vec, idx| {
                let idx = idx.into_u16()? as usize;
                Some(Value::Int(vec.into_bytes()?.get(idx)?.clone().into()))
            })?,
            OpCode::BSet => self.do_triop(|vec, idx, value| {
                let idx = idx.into_u16()? as usize;
                let mut vec = vec.into_bytes()?;
                if idx < vec.len() {
                    vec.set(idx, value.into_truncated_u8()?);
                    Some(Value::Bytes(vec))
                } else {
                    None
                }
            })?,
            OpCode::BAppend => self.do_binop(|v1, v2| {
                let mut v1 = v1.into_bytes()?;
                let v2 = v2.into_bytes()?;
                v1.append(v2);
                Some(Value::Bytes(v1))
            })?,
            OpCode::BSlice => self.do_triop(|vec, i, j| {
                let i = i.into_u16()? as usize;
                let j = j.into_u16()? as usize;
                match vec {
                    Value::Bytes(mut vec) => {
                        if j >= vec.len() || j <= i {
                            Some(Value::Bytes(im::Vector::new()))
                        } else {
                            Some(Value::Bytes(vec.slice(i..j)))
                        }
                    }
                    _ => None,
                }
            })?,
            OpCode::BLength => self.do_monop(|vec| match vec {
                Value::Bytes(vec) => Some(Value::Int(U256::from(vec.len() as u64))),
                _ => None,
            })?,
            // control flow
            OpCode::Bez(jgap) => {
                let top = self.stack.pop()?;
                if top == Value::Int(0u32.into()) {
                    return Some(1 + *jgap as usize);
                }
            }
            OpCode::Bnz(jgap) => {
                let top = self.stack.pop()?;
                if top != Value::Int(0u32.into()) {
                    return Some(1 + *jgap as usize);
                }
            }
            OpCode::Jmp(jgap) => {
                return Some(1 + *jgap as usize);
            }
            OpCode::Loop(iterations, op_count) => {
                if *iterations > 0 && *op_count > 0 {
                    self.loop_state.push(LoopState {
                        begin: self.pc + 1,
                        end: self.pc + *op_count as usize,
                        iterations_left: *iterations,
                    });
                } else {
                    return None;
                }
            }
            // Conversions
            OpCode::BtoI => self.do_monop(|x| {
                let bytes = x.into_bytes()?;
                let bytes: [u8; 32] = bytes.into_iter().collect::<Vec<_>>().try_into().ok()?;

                Some(Value::Int(U256::from_be_bytes(bytes)))
            })?,
            OpCode::ItoB => self.do_monop(|x| {
                let n = x.into_int()?;
                Some(Value::Bytes(n.to_be_bytes().iter().copied().collect()))
            })?,
            // literals
            OpCode::PushB(bts) => {
                let bts = Value::from_bytes(bts);
                self.stack.push(bts);
            }
            OpCode::PushI(num) => self.stack.push(Value::Int(*num)),
            OpCode::TypeQ => self.do_monop(|x| match x {
                Value::Int(_) => Some(Value::Int(0u32.into())),
                Value::Bytes(_) => Some(Value::Int(1u32.into())),
                Value::Vector(_) => Some(Value::Int(2u32.into())),
            })?,
            // dup
            OpCode::Dup => {
                let val = self.stack.pop()?;
                self.stack.push(val.clone());
                self.stack.push(val);
            }
        }

        // Default is to step pc by one
        Some(1)
    }
    fn run_bare(&mut self, ops: &[OpCode]) -> Option<()> {
        assert!(ops.len() < 512 * 1024);
        // Run to completion
        let weight = opcodes_weight(&ops);
        let mut steps = 0u128;
        while self.pc < ops.len() {
            if steps > weight {
                log::error!("{:?}", ops);
                panic!("somehow exceeded weight {}", weight)
            }
            self.step(ops.get(self.pc)?)?;
            steps += 1;
        }

        Some(())
    }
    fn run_return(&mut self, ops: &[OpCode]) -> bool {
        self.run_bare(ops);
        let res = self.stack.pop();
        !(res == None || res == Some(Value::Int(U256::from(0u32))))
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
    Shl,
    Shr,
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
    VAppend,
    VEmpty,
    VLength,
    VSlice,
    VSet,
    VPush,
    VCons,
    // bytes operations
    BRef,
    BAppend,
    BEmpty,
    BLength,
    BSlice,
    BSet,
    BPush,
    BCons,

    // control flow
    Bez(u16),
    Bnz(u16),
    Jmp(u16),
    // Loop(iterations, instructions)
    Loop(u16, u16),

    // type conversions
    ItoB,
    BtoI,
    TypeQ,
    // SERIAL(u16),

    // literals
    PushB(Vec<u8>),
    PushI(U256),

    // duplication
    Dup,
}

/// Computes the weight of a bunch of opcodes.
fn opcodes_weight(opcodes: &[OpCode]) -> u128 {
    let (mut sum, mut rest) = opcodes_car_weight(opcodes);
    while !rest.is_empty() {
        let (delta_sum, new_rest) = opcodes_car_weight(rest);
        rest = new_rest;
        sum = sum.saturating_add(delta_sum);
    }
    sum
}

/// Compute the weight of the first bit of opcodes, returning a weight and what remains.
fn opcodes_car_weight(opcodes: &[OpCode]) -> (u128, &[OpCode]) {
    if opcodes.is_empty() {
        return (0, opcodes);
    }
    let (first, rest) = opcodes.split_first().unwrap();
    match first {
        // handle loops specially
        OpCode::Loop(iters, body_len) => {
            let mut sum = 0u128;
            let mut rest = rest;
            for _ in 0..*body_len {
                let (weight, rem) = opcodes_car_weight(rest);
                sum = sum.saturating_add(weight);
                rest = rem;
            }
            (sum.saturating_mul(*iters as u128).saturating_add(1), rest)
        }
        OpCode::Add => (4, rest),
        OpCode::Sub => (4, rest),
        OpCode::Mul => (6, rest),
        OpCode::Div => (6, rest),
        OpCode::Rem => (6, rest),

        OpCode::And => (4, rest),
        OpCode::Or => (4, rest),
        OpCode::Xor => (4, rest),
        OpCode::Not => (4, rest),
        OpCode::Eql => (4, rest),
        OpCode::Lt => (4, rest),
        OpCode::Gt => (4, rest),
        OpCode::Shl => (4, rest),
        OpCode::Shr => (4, rest),

        OpCode::Hash(n) => (50u128.saturating_add(*n as u128), rest),
        OpCode::SigEOk(n) => (100u128.saturating_add(*n as u128), rest),

        OpCode::Store => (10, rest),
        OpCode::Load => (10, rest),
        OpCode::StoreImm(_) => (4, rest),
        OpCode::LoadImm(_) => (4, rest),

        OpCode::VRef => (10, rest),
        OpCode::VSet => (20, rest),
        OpCode::VAppend => (50, rest),
        OpCode::VSlice => (50, rest),
        OpCode::VLength => (4, rest),
        OpCode::VEmpty => (4, rest),
        OpCode::BEmpty => (4, rest),
        OpCode::BPush => (10, rest),
        OpCode::VPush => (10, rest),
        OpCode::VCons => (10, rest),
        OpCode::BRef => (10, rest),
        OpCode::BAppend => (10, rest),
        OpCode::BLength => (4, rest),
        OpCode::BSlice => (50, rest),
        OpCode::BSet => (20, rest),
        OpCode::BCons => (10, rest),

        OpCode::TypeQ => (4, rest),

        OpCode::ItoB => (50, rest),
        OpCode::BtoI => (50, rest),

        OpCode::Bez(_) => (1, rest),
        OpCode::Bnz(_) => (1, rest),
        OpCode::Jmp(_) => (1, rest),

        OpCode::PushB(_) => (1, rest),
        OpCode::PushI(_) => (1, rest),

        OpCode::Dup => (4, rest),
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub enum Value {
    Int(U256),
    Bytes(im::Vector<u8>),
    Vector(im::Vector<Value>),
}

impl Value {
    fn into_int(self) -> Option<U256> {
        match self {
            Value::Int(bi) => Some(bi),
            _ => None,
        }
    }
    fn into_u16(self) -> Option<u16> {
        let num = self.into_int()?;
        if num > U256::from(65535u32) {
            None
        } else {
            Some(*num.low() as u16)
        }
    }
    fn into_truncated_u8(self) -> Option<u8> {
        let num = self.into_int()?;
        Some(*num.low() as u8)
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
            Value::Int(1u32.into())
        } else {
            Value::Int(0u32.into())
        }
    }

    fn into_bytes(self) -> Option<im::Vector<u8>> {
        match self {
            Value::Bytes(bts) => Some(bts),
            _ => None,
        }
    }

    fn into_vector(self) -> Option<im::Vector<Value>> {
        match self {
            Value::Vector(vec) => Some(vec),
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

impl From<Denom> for Value {
    fn from(v: Denom) -> Self {
        Value::Bytes(v.to_bytes().into_iter().collect::<im::Vector<u8>>())
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v.into_iter().collect::<im::Vector<u8>>())
    }
}

impl From<HexBytes> for Value {
    fn from(v: HexBytes) -> Self {
        Value::Bytes(v.0.into_iter().collect::<im::Vector<u8>>())
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
        let check_sig_script = Covenant::from_ops(&[
            OpCode::Loop(5, 8),
            OpCode::PushI(0u32.into()),
            OpCode::PushI(6u32.into()),
            OpCode::LoadImm(0),
            OpCode::VRef,
            OpCode::VRef,
            OpCode::PushB(pk.0.to_vec()),
            OpCode::LoadImm(1),
            OpCode::SigEOk(32),
        ])
        .unwrap();
        println!("script length is {}", check_sig_script.0.len());
        let mut tx = Transaction::empty_test().signed_ed25519(sk);
        assert!(check_sig_script.check_no_env(&tx));
        tx.sigs[0][0] ^= 123;
        assert!(!check_sig_script.check_no_env(&tx));
    }

    // #[quickcheck]
    // fn loop_once_is_identity(bitcode: Vec<u8>) -> bool {
    //     let ops = Covenant(bitcode.clone()).to_ops();
    //     let tx = Transaction::empty_test();
    //     match ops {
    //         None => true,
    //         Some(ops) => {
    //             let loop_ops = vec![OpCode::Loop(1, ops.clone())];
    //             let loop_script = Covenant::from_ops(&loop_ops).unwrap();
    //             let orig_script = Covenant::from_ops(&ops).unwrap();
    //             loop_script.check_no_env(&tx) == orig_script.check_no_env(&tx)
    //         }
    //     }
    // }

    #[quickcheck]
    fn deterministic_execution(bitcode: Vec<u8>) -> bool {
        let ops = Covenant(bitcode.clone()).to_ops();
        let tx = Transaction::empty_test();
        match ops {
            None => true,
            Some(ops) => {
                let orig_script = Covenant::from_ops(&ops).unwrap();
                let first = orig_script.check_no_env(&tx);
                let second = orig_script.check_no_env(&tx);
                first == second
            }
        }
    }
}
