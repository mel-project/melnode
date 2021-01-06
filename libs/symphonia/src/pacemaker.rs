use crate::{common::*, Decider};
use crate::{machine::Machine, DestMsg};
use async_trait::async_trait;
use log::trace;
use smol::channel::{Receiver, Sender};
use smol::prelude::*;
use smol_timeout::TimeoutExt;
use std::ops::DerefMut;
use std::time::Duration;

/// A Pacemaker is an implementation of Decider that uses a Machine to decide on a value.
pub struct Pacemaker {
    send_input: Sender<SignedMessage>,
    recv_output: Receiver<DestMsg>,
    decision_output: smol::lock::Mutex<smol::Task<QuorumCert>>,
}

impl Pacemaker {
    /// Creates a new Pacemaker from a consumed Machine.
    pub fn new(machine: Machine) -> Self {
        let (send_input, recv_input) = smol::channel::unbounded();
        let (send_output, recv_output) = smol::channel::unbounded();
        Pacemaker {
            send_input,
            recv_output,
            decision_output: smol::lock::Mutex::new(smolscale::spawn(async move {
                pacemaker_loop(machine, recv_input, send_output).await
            })),
        }
    }
}

#[async_trait]
impl Decider for Pacemaker {
    // Next output message.
    async fn next_output(&self) -> DestMsg {
        match self.recv_output.recv().await {
            Ok(msg) => msg,
            _ => smol::future::pending().await,
        }
    }

    /// Final decision, represented as a quorum certificate.
    async fn decision(&self) -> QuorumCert {
        let mut out = self.decision_output.lock().await;
        out.deref_mut().await
    }

    /// Processes an input message.
    fn process_input(&self, msg: SignedMessage) {
        let _ = self.send_input.try_send(msg);
    }
}

async fn pacemaker_loop(
    mut machine: Machine,
    mut recv_input: Receiver<SignedMessage>,
    send_output: Sender<DestMsg>,
) -> QuorumCert {
    trace!("pacemaker started");
    let mut timeout = Duration::from_millis(5000);
    loop {
        let tt = timeout;
        // let mut timeout_chan = async move { smol::Timer::after(tt).await }.boxed().fuse();
        //thread::sleep_ms(1000);
        // send outputs
        let outputs = machine.drain_output();
        for msg in outputs {
            // trace!("machine send {:?}", msg.1.msg);
            let _ = send_output.send(msg).await;
        }
        if let Some(dec) = machine.decision() {
            trace!("pacemaker stopped because decision reached");
            return dec;
        }
        // wait for input, or timeout
        let recieved_input = recv_input.next().timeout(tt).await;
        if let Some(opt_msg) = recieved_input {
            if let Some(signed_msg) = opt_msg {
                trace!("machine process {:?}", signed_msg.msg.phase);
                machine.process_input(signed_msg.clone());
            } else {
                panic!("pacemaker stopped because recv_input dead");
            }
        } else {
            trace!("pacemaker forcing a new view after {:?}", timeout);
            timeout = timeout * 10 / 9;
            trace!("new timeout {:?}", timeout);
            machine.new_view();
        }
    }
}
