use blkstructs::Transaction;
use std::{net::SocketAddr, sync::Arc};

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

        network.register_verb("send_tx", {
            let responder = responder.clone();
            move |_, tx| {
                let responder = responder.clone();
                async move { responder.resp_send_tx(tx).await }
            }
        });
        network.register_verb("get_state", {
            let responder = responder.clone();
            move |_, _: ()| {
                let responder = responder.clone();
                async move { responder.resp_get_state().await }
            }
        });
        network.register_verb("get_txx", {
            let responder = responder.clone();
            move |_, txx| {
                let responder = responder.clone();
                async move { responder.resp_get_txx(txx).await }
            }
        });
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
        unimplemented!()
    }

    /// Attempts to synchronize the latest state from the network. If nobody else has the best state, return our own state.
    pub fn pull_state(&mut self) -> anyhow::Result<AuditorState> {
        unimplemented!()
    }

    /// Forces the AuditorProtocol to adopt a certain state. This is generally only called by stakeholders to "bridge" the two p2p networks.
    pub fn force_state(&mut self) -> anyhow::Result<AuditorState> {
        unimplemented!()
    }
}

pub struct AuditorState();

pub struct AuditorResponder {
    state: AuditorState,
}

impl AuditorResponder {
    fn new(state: AuditorState) -> Self {
        Self { state }
    }

    async fn resp_send_tx(&self, tx: Transaction) -> melnet::Result<()> {
        unimplemented!()
    }

    async fn resp_get_state(&self) -> melnet::Result<()> {
        unimplemented!()
    }

    async fn resp_get_txx(&self, txx: Vec<Transaction>) -> melnet::Result<()> {
        unimplemented!()
    }
}
