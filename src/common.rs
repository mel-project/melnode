use anyhow::Result;
use futures::channel::mpsc;
use futures::prelude::*;
use smol::*;
use std::net::{TcpListener, ToSocketAddrs};
//use std::pin::Pin;

//pub type PinBoxFut<T> = Pin<Box<dyn Future<Output = T> + 'static>>;

/// Guesses the public IP address of the current machine.
async fn guess_my_ip() -> Result<String> {
    // TODO: something better-quality
    let response = blocking!(attohttpc::get("http://icanhazip.org").send())?;
    Ok(response.text()?.trim().to_owned())
}

/// Actor is a wrapper around a smol Task that comes with an asynchronous mailbox similar to that of Erlang. Unlike with channels, using the mailbox is infallible, because the sender can only be dropped if the task is dropped, yet if the task is dropped it is cancelled. If the Actor needs to be cloned, wrap in an Arc.
#[derive(Debug)]
pub struct Actor<ChType> {
    sender: mpsc::UnboundedSender<ChType>,
    _task: Option<Task<()>>,
}

impl<ChType> Drop for Actor<ChType> {
    fn drop(&mut self) {
        let task = std::mem::replace(&mut self._task, None).unwrap();
        block_on(task.cancel());
    }
}

impl<ChType> Actor<ChType> {
    /// Spawn spawns a new Actor.
    pub fn spawn<T: Future<Output = ()> + 'static + Send, F: FnOnce(Mailbox<ChType>) -> T>(
        closure: F,
    ) -> Self {
        let (send, recv) = mpsc::unbounded();
        let fut = closure(Mailbox(recv));
        let task = Task::spawn(fut);
        Actor {
            sender: send,
            _task: Some(task),
        }
    }

    /// Sends a message to the Actor.
    pub fn send(&self, msg: ChType) {
        self.sender
            .unbounded_send(msg)
            .expect("mailbox send invariant failed?!")
    }
}

/// Mailbox is an opaque mailbox passed into the closure that an Actor runs.
pub struct Mailbox<T>(mpsc::UnboundedReceiver<T>);

impl<T> Mailbox<T> {
    /// Receives the next message from the mailbox.
    pub async fn recv(&mut self) -> T {
        self.0
            .next()
            .await
            .expect("mailbox recv invariant failed?!")
    }
}

/// Creates a new melnet state with a default route.
pub async fn new_melnet(listener: &Async<TcpListener>, name: &str) -> Result<melnet::NetState> {
    let my_ip = guess_my_ip().await?;
    let my_ip_port = format!("{}:{}", my_ip, listener.get_ref().local_addr()?.port());
    let net = melnet::NetState::new_with_name(name);
    net.add_route(my_ip_port.to_socket_addrs()?.next().unwrap());
    Ok(net)
}
