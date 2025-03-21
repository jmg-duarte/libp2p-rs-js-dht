use libp2p::{core, Multiaddr, PeerId};

pub fn extract_peer_id(maddr: &Multiaddr) -> Option<PeerId> {
    match maddr.iter().last() {
        Some(core::multiaddr::Protocol::P2p(peer_id)) => Some(peer_id),
        _ => None,
    }
}
