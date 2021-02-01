use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Serialize};
use smol::channel::Sender;
use smol::prelude::*;

use crate::MelnetError;

/// A Responder responds to Requests. Requests are responded to by calling `Request::respond()` rather than by return value to simplify asynchronous handling.
pub trait Responder<Req: DeserializeOwned, Resp: Serialize> {
    /// Handle a request. This should not block. Implementations should do things like move the Request to background tasks/threads to avoid this.
    fn respond(&mut self, req: Request<Req, Resp>);
}

/// Creates an anonymous responder.
pub fn anon_responder<
    Req: DeserializeOwned + 'static + Send,
    Resp: Serialize + 'static + Send,
    F: FnMut(Request<Req, Resp>) + 'static + Send,
>(
    closure: F,
) -> impl Responder<Req, Resp> + 'static + Send {
    AnonResponder {
        clos: closure,
        _a: PhantomData::default(),
        _b: PhantomData::default(),
    }
}

/// Anonymous responder.
struct AnonResponder<
    Req: DeserializeOwned + 'static + Send,
    Resp: Serialize + 'static + Send,
    F: FnMut(Request<Req, Resp>) + 'static + Send,
> {
    clos: F,
    _a: PhantomData<Req>,
    _b: PhantomData<Resp>,
}

impl<
        Req: DeserializeOwned + 'static + Send,
        Resp: Serialize + 'static + Send,
        F: FnMut(Request<Req, Resp>) + 'static + Send,
    > Responder<Req, Resp> for AnonResponder<Req, Resp, F>
{
    fn respond(&mut self, req: Request<Req, Resp>) {
        (self.clos)(req)
    }
}

/// Converts a responder to a boxed closure for internal use.
pub(crate) fn responder_to_closure<
    Req: DeserializeOwned + Send,
    Resp: Serialize + Send + 'static,
>(
    state: crate::NetState,
    mut responder: impl Responder<Req, Resp> + 'static + Send,
) -> BoxedResponder {
    let clos = move |bts: &[u8]| {
        let decoded: Result<Req, _> = stdcode::deserialize(&bts);
        match decoded {
            Ok(decoded) => {
                let (respond, recv_respond) = smol::channel::bounded(1);
                let request = Request {
                    state: state.clone(),
                    body: decoded,
                    respond,
                };
                responder.respond(request);
                let response_fut = async move {
                    recv_respond
                        .recv()
                        .await
                        .unwrap()
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
    BoxedResponder(Box::new(clos))
}

#[allow(clippy::type_complexity)]
pub(crate) struct BoxedResponder(
    pub Box<dyn FnMut(&[u8]) -> smol::future::Boxed<crate::Result<Vec<u8>>> + Send>,
);

/// A `Request<Req, Resp>` carries a stdcode-compatible request of type `Req and can be responded to with responses of type Resp.
pub struct Request<Req: DeserializeOwned, Resp: Serialize> {
    pub body: Req,
    pub state: crate::NetState,
    respond: Sender<crate::Result<Resp>>,
}

impl<Req: DeserializeOwned, Resp: Serialize> Drop for Request<Req, Resp> {
    fn drop(&mut self) {
        let _ = self
            .respond
            .try_send(Err(crate::MelnetError::InternalServerError));
    }
}

impl<Req: DeserializeOwned, Resp: Serialize> Request<Req, Resp> {
    /// Respond to a Request
    pub fn respond(self, resp: crate::Result<Resp>) {
        let _ = self.respond.try_send(resp);
    }
}
