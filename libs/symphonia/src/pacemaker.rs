use crate::common::*;
use crate::machine::Machine;
use futures::channel::mpsc;
use futures::lock;
use futures::prelude::*;
use futures::select;
use log::trace;
use parking_lot::Mutex;

use std::ops::DerefMut;
use std::time::Duration;

type DestMsg = (Option<tmelcrypt::Ed25519PK>, SignedMessage);
pub struct Pacemaker {
    msg_input: Mutex<mpsc::UnboundedSender<SignedMessage>>,
    msg_output: lock::Mutex<mpsc::Receiver<DestMsg>>,
    decision_output: lock::Mutex<smol::Task<QuorumCert>>,
}

impl Pacemaker {
    // Creates a new Pacemaker from a consumed Machine.
    pub fn new(machine: Machine) -> Self {
        let (send_input, recv_input) = mpsc::unbounded();
        let (send_output, recv_output) = mpsc::channel(0);
        Pacemaker {
            msg_input: Mutex::new(send_input),
            msg_output: lock::Mutex::new(recv_output),
            decision_output: lock::Mutex::new(smol::Task::spawn(async move {
                pacemaker_loop(machine, recv_input, send_output).await
            })),
        }
    }

    // Next output message.
    pub async fn next_output(&self) -> DestMsg {
        match self.msg_output.lock().await.next().await {
            Some(msg) => msg,
            None => future::pending().await,
        }
    }

    // Final decision, represented as a quorum certificate.
    pub async fn decision(&self) -> QuorumCert {
        let mut out = self.decision_output.lock().await;
        out.deref_mut().await
    }

    // Processes an input message.
    pub fn process_input(&self, msg: SignedMessage) {
        let _ = self.msg_input.lock().unbounded_send(msg);
    }
}

async fn pacemaker_loop(
    mut machine: Machine,
    mut recv_input: mpsc::UnboundedReceiver<SignedMessage>,
    mut send_output: mpsc::Sender<DestMsg>,
) -> QuorumCert {
    trace!("pacemaker started");
    let mut timeout = Duration::from_millis(5000);
    let mut timeout_chan = smol::Timer::new(timeout).fuse();
    loop {
        //thread::sleep_ms(1000);
        // send outputs
        let outputs = machine.drain_output();
        for msg in outputs {
            // trace!("machine send {:?}", msg.1.msg.phase);
            let _ = send_output.send(msg).await;
        }
        if let Some(dec) = machine.decision() {
            trace!("pacemaker stopped because decision reached");
            return dec;
        }
        // wait for input, or timeout
        select! {
            s_msg = recv_input.next() => {
                if let Some(s_msg) = s_msg {
                    //trace!("machine process {:?}", s_msg.msg.phase);
                    machine.process_input(s_msg.clone());
                } else {
                    panic!("pacemaker stopped because recv_input dead");
                }
            }
         _ = timeout_chan => {
                trace!("pacemaker forcing a new view after {:?}", timeout);
                timeout = timeout * 10 / 9;
                machine.new_view();
                timeout_chan = smol::Timer::new(timeout).fuse();
            }
        }
    }
}
