use std::{net::SocketAddr, sync::Arc};

use blkstructs::Transaction;

/// AuditorProtocol encapsulates the auditor peer-to-peer.
pub struct AuditorProtocol {
    network: melnet::NetState,
    responder: Arc<AuditorResponder>,
    _network_task: smol::Task<()>,
}

impl AuditorProtocol {
    /// Creates a new AuditorProtocol listening on the given address with the given AuditorState.
    pub fn new(addr: SocketAddr, state: AuditorState) -> anyhow::Result<Self> {
        let network = melnet::NetState::new_with_name("testnet-auditor");
        let responder = Arc::new(AuditorResponder::new(state));

        let rr = responder.clone();
        network.register_verb(
            "send_tx",
            melnet::anon_responder(move |req: melnet::Request<Transaction, _>| {
                let txn = &req.body;
                req.respond(rr.resp_send_tx(txn.clone()))
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_state",
            melnet::anon_responder(move |req: melnet::Request<(), _>| {
                req.respond(rr.resp_get_state())
            }),
        );
        let rr = responder.clone();
        network.register_verb(
            "get_txx",
            melnet::anon_responder(move |req| req.respond(rr.resp_get_txx(req.body))),
        );
        let net2 = network.clone();
        let _network_task = smolscale::spawn(async move {
            net2.run_server(smol::net::TcpListener::bind(addr).await.unwrap())
                .await
        });
        Ok(Self {
            network,
            responder,
            _network_task,
        })
    }

    /// Broadcasts a transaction into the network.
    pub fn broadcast(&self, txn: Transaction) -> anyhow::Result<()> {
        Ok(self.responder.resp_send_tx(txn)?)
    }

    /// Attempts to synchronize the latest state from the network. If nobody else has the best state, return our own state.
    pub fn pull_state(&mut self) {
        unimplemented!()
    }

    /// Forces the AuditorProtocol to adopt a certain state. This is generally only called by stakeholders to "bridge" the two p2p networks.
    pub fn force_state(&mut self) -> anyhow::Result<AuditorState> {
        unimplemented!()
    }
}

/// This structure encapsulates an auditor state.
pub struct AuditorState {
    storage: Arc<crate::Storage>,
}

pub struct AuditorResponder {
    state: AuditorState,
}

impl AuditorResponder {
    fn new(state: AuditorState) -> Self {
        Self { state }
    }

    fn resp_send_tx(&self, tx: Transaction) -> melnet::Result<()> {
        unimplemented!()
    }

    fn resp_get_state(&self) -> melnet::Result<()> {
        unimplemented!()
    }

    fn resp_get_txx(&self, txx: Vec<tmelcrypt::HashVal>) -> melnet::Result<()> {
        unimplemented!()
    }
}
