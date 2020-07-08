use log::debug;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct RoutingTable {
    addr_last_seen: HashMap<SocketAddr, Instant>,
}

impl RoutingTable {
    /// Adds a route to the routing table, asserting that the route is up to date.
    pub fn add_route(&mut self, addr: SocketAddr) {
        self.clean_up();
        self.addr_last_seen.insert(addr, Instant::now());
    }

    /// Cleans up really old routes.
    fn clean_up(&mut self) {
        let to_del: Vec<_> = self
            .addr_last_seen
            .clone()
            .into_iter()
            .filter(|(_, start)| *start < Instant::now() - Duration::from_secs(600))
            .collect();
        for (del, inst) in to_del {
            debug!("removing {:?}:{:?}", del, inst);
            self.addr_last_seen.remove(&del);
        }
    }

    /// Iterates over the routes.
    pub fn iter(&self) -> impl Iterator<Item = (&SocketAddr, &Instant)> {
        self.addr_last_seen.iter()
    }
}
