use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use smol::channel::Sender;
use smol::prelude::*;

use crate::MelnetError;

/// An Endpoint responds to Requests. Requests are responded to by calling `Request::respond()` rather than by return value to simplify asynchronous handling.
pub trait Endpoint<Req: DeserializeOwned, Resp: Serialize>: Send + Sync {
    /// Handle a request. This should not block. Implementations should do things like move the Request to background tasks/threads to avoid this.
    fn respond(&self, req: Request<Req, Resp>);
}

impl<Req: DeserializeOwned, Resp: Serialize, F: Fn(Request<Req, Resp>) + 'static + Send + Sync>
    Endpoint<Req, Resp> for F
{
    fn respond(&self, req: Request<Req, Resp>) {
        (self)(req)
    }
}

/// Converts a responder to a boxed closure for internal use.
pub(crate) fn responder_to_closure<
    Req: DeserializeOwned + Send,
    Resp: Serialize + Send + 'static,
>(
    state: crate::NetState,
    responder: impl Endpoint<Req, Resp> + 'static + Send,
) -> BoxedResponder {
    let clos = move |bts: &[u8]| {
        let decoded: Result<Req, _> = stdcode::deserialize(&bts);
        match decoded {
            Ok(decoded) => {
                let (respond, recv_respond) = smol::channel::bounded(1);
                let request = Request {
                    state: state.clone(),
                    body: decoded,
                    response: ResponseChan { respond },
                };
                responder.respond(request);
                let response_fut = async move {
                    recv_respond
                        .recv()
                        .await
                        .unwrap_or(Err(MelnetError::InternalServerError))
                        .map(|v| stdcode::serialize(&v).unwrap())
                };
                response_fut.boxed()
            }
            Err(e) => {
                log::warn!("issue decoding request: {}", e);
                async { Err(MelnetError::InternalServerError) }.boxed()
            }
        }
    };
    BoxedResponder(Arc::new(clos))
}

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub(crate) struct BoxedResponder(
    pub Arc<dyn Fn(&[u8]) -> smol::future::Boxed<crate::Result<Vec<u8>>> + Send + Sync>,
);

/// A `Request<Req, Resp>` carries a stdcode-compatible request of type `Req and can be responded to with responses of type Resp.
#[must_use]
pub struct Request<Req: DeserializeOwned, Resp: Serialize> {
    pub body: Req,
    pub state: crate::NetState,
    pub response: ResponseChan<Resp>,
}
/// A single-use channel through which to send a response.
pub struct ResponseChan<Resp: Serialize> {
    respond: Sender<crate::Result<Resp>>,
}

impl<Resp: Serialize> ResponseChan<Resp> {
    /// Respond to a Request
    pub fn send(self, resp: crate::Result<Resp>) {
        let _ = self.respond.try_send(resp);
    }
}
