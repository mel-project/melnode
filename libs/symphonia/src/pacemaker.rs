use crate::common::*;
use crate::machine::Machine;
use crossbeam_channel::*;
use log::trace;
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

type DestMsg = (Option<tmelcrypt::Ed25519PK>, SignedMessage);
pub struct Pacemaker {
    machine: Arc<Mutex<Machine>>,
    msg_input: Sender<SignedMessage>,
    msg_output: Receiver<DestMsg>,
    decision: Receiver<QuorumCert>,
}

impl Pacemaker {
    // Creates a new Pacemaker from a Machine.
    pub fn new(machine: Machine) -> Self {
        let (send_input, recv_input) = unbounded();
        let (send_output, recv_output) = bounded(0);
        let (send_dec, recv_desc) = unbounded();
        let pace = Pacemaker {
            machine: Arc::new(Mutex::new(machine)),
            msg_input: send_input,
            msg_output: recv_output,
            decision: recv_desc,
        };
        let machine = Arc::clone(&pace.machine);
        thread::spawn(move || {
            pacemaker_loop(machine, recv_input, send_output, send_dec);
        });
        pace
    }

    // Returns a channel of output messages.
    pub fn output_chan(&self) -> &Receiver<DestMsg> {
        &self.msg_output
    }

    // Returns a one-off channel that returns the decision.
    pub fn decision(&self) -> &Receiver<QuorumCert> {
        &self.decision
    }

    // Processes an input message.
    pub fn process_input(&mut self, msg: SignedMessage) {
        let _ = self.msg_input.try_send(msg);
    }
}

fn pacemaker_loop(
    machine: Arc<Mutex<Machine>>,
    recv_input: Receiver<SignedMessage>,
    send_output: Sender<DestMsg>,
    send_dec: Sender<QuorumCert>,
) {
    trace!("pacemaker started");
    let mut timeout = Duration::from_millis(5000);
    let mut timeout_chan = after(timeout);
    loop {
        //thread::sleep_ms(1000);
        // send outputs
        for msg in machine.lock().drain_output() {
            // trace!("machine send {:?}", msg.1.msg.phase);
            send_output.send(msg).unwrap();
        }
        if let Some(dec) = machine.lock().decision() {
            send_dec.send(dec).unwrap();
            trace!("pacemaker stopped because decision reached");
            return;
        }
        // wait for input, or timeout
        select! {
            recv(recv_input) -> s_msg => {
                if let Ok(s_msg) = s_msg {
                    //trace!("machine process {:?}", s_msg.msg.phase);
                    machine.lock().process_input(s_msg.clone());
                } else {
                    trace!("pacemaker stopped because recv_input dead");
                    return
                }
            }
            recv(timeout_chan) -> _ => {
                trace!("pacemaker forcing a new view after {:?}", timeout);
                timeout = timeout * 10 / 9;
                machine.lock().new_view();
                timeout_chan = after(timeout);
            }
        }
    }
}
