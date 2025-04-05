pub mod lp;

use libp2p::{core, Multiaddr, PeerId};
use serde::{Deserialize, Serialize};

pub fn extract_peer_id(maddr: &Multiaddr) -> Option<PeerId> {
    match maddr.iter().last() {
        Some(core::multiaddr::Protocol::P2p(peer_id)) => Some(peer_id),
        _ => None,
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub peer: PeerId,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Response {
    Found {
        peer: PeerId,
        maddrs: Vec<Multiaddr>,
    },
    NotFound {
        peer: PeerId,
    },
}
