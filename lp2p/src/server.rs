use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use clap::Parser;
use libp2p::{
    autonat, core,
    futures::StreamExt,
    identify,
    identity::{self, Keypair},
    kad::{self, InboundRequest, QueryId, QueryResult, Quorum, Record},
    noise, ping,
    request_response::{self, ProtocolSupport},
    swarm::{self, NetworkBehaviour, SwarmEvent},
    tcp, websocket, yamux, Multiaddr, PeerId, StreamProtocol, Transport,
};
use lp2p::{extract_peer_id, lp::LpCbor, Request, Response};
use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Clone, Debug, clap::Parser)]
struct App {
    #[arg(short='l', value_delimiter=',', num_args=1.., default_value = "/ip4/0.0.0.0/tcp/64001,/ip4/0.0.0.0/tcp/64002/ws")]
    listen_addrs: Vec<Multiaddr>,

    #[arg(short='b', value_delimiter=',', num_args=1..)]
    bootnodes: Vec<Multiaddr>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::DEBUG.into())
                .from_env()
                .unwrap(),
        )
        .init();

    let app = App::parse();

    Swarm::new(app.bootnodes).start(app.listen_addrs).await;
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    identify: identify::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
    rr: request_response::Behaviour<LpCbor<Request, Response>>,
}

impl Behaviour {
    fn new(keypair: Keypair, bootnodes: Vec<Multiaddr>) -> Self {
        let identify = identify::Behaviour::new(identify::Config::new(
            "/polka-test/identify/1.0.0".to_string(),
            keypair.public(),
        ));

        let local_peer_id = keypair.public().to_peer_id();
        let mut kad =
            kad::Behaviour::new(local_peer_id, kad::store::MemoryStore::new(local_peer_id));
        kad.set_mode(Some(kad::Mode::Server));

        for node in bootnodes {
            tracing::info!("Adding address to Kademlia: {node}");
            kad.add_address(&extract_peer_id(&node).unwrap(), node);
        }

        let rr = request_response::Behaviour::new(
            [(StreamProtocol::new("/rr/1.0.0"), ProtocolSupport::Inbound)],
            Default::default(),
        );

        Self { identify, kad, rr }
    }
}

struct Swarm {
    inner: libp2p::Swarm<Behaviour>,
}

// Unsure about the ergonomics vs readability on this one, but let's see
impl Deref for Swarm {
    type Target = libp2p::Swarm<Behaviour>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Swarm {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Swarm {
    fn new(bootnodes: Vec<Multiaddr>) -> Self {
        let identity = identity::Keypair::generate_ed25519();
        let local_peer_id = identity.public().to_peer_id();
        tracing::info!("Local peer id: {local_peer_id}");

        let noise_config = noise::Config::new(&identity).unwrap(); // TODO: proper error handling
        let muxer_config = yamux::Config::default();

        let tcp_config = tcp::Config::new();
        let tcp_transport = tcp::tokio::Transport::new(tcp_config.clone());

        let ws = websocket::WsConfig::new(tcp::tokio::Transport::new(tcp_config));
        let tcp_ws_transport = ws
            .or_transport(tcp_transport)
            .upgrade(core::upgrade::Version::V1)
            .authenticate(noise_config)
            .multiplex(muxer_config)
            .boxed();

        let local_peer_id = identity.public().to_peer_id();
        let swarm = libp2p::Swarm::new(
            tcp_ws_transport,
            Behaviour::new(identity, bootnodes),
            local_peer_id,
            swarm::Config::with_tokio_executor(),
        );

        Swarm { inner: swarm }
    }

    async fn start(mut self, listen_addrs: Vec<Multiaddr>) {
        for addr in listen_addrs {
            if let Err(err) = self.listen_on(addr) {
                // Assuming that the error has the failed maddr
                tracing::error!("Failed to listen on address with error: {err}");
            }
        }
        loop {
            let event = self.select_next_some().await;
            self.on_swarm_event(event);
        }
    }

    fn on_swarm_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                tracing::debug!("New listen address: {address}");
            }
            SwarmEvent::ExternalAddrConfirmed { address } => {
                tracing::debug!("Local external address confirmed: {address}")
            }
            SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
                tracing::debug!("External address confirmed: {address} for {peer_id}")
            }
            SwarmEvent::IncomingConnectionError { .. }
            | SwarmEvent::ListenerError { .. }
            | SwarmEvent::OutgoingConnectionError { .. } => {
                tracing::error!("Received unhandled error: {event:?}")
            }
            SwarmEvent::Behaviour(event) => self.on_behaviour_event(event),
            _ => tracing::debug!("Received unhandled event: {event:?}"),
        }
    }

    fn on_behaviour_event(&mut self, event: BehaviourEvent) {
        match event {
            BehaviourEvent::Rr(event) => match event {
                request_response::Event::Message {
                    peer,
                    connection_id,
                    message,
                } => match message {
                    request_response::Message::Request {
                        request_id,
                        request,
                        channel,
                    } => {
                        let query_id = self.behaviour_mut().kad.kbucket(request.peer);
                        match query_id {
                            Some(k_ref) => {
                                let maybe_entry = k_ref
                                    .iter()
                                    .inspect(|entry| {
                                        tracing::debug!(
                                            "KBucket Entries for {}: {} @ {:?}",
                                            request.peer,
                                            entry.node.key.preimage(),
                                            entry.node.value
                                        );
                                    })
                                    .filter(|entry| entry.node.key.preimage() == &request.peer)
                                    .take(1)
                                    .collect::<Vec<_>>()
                                    .pop();

                                if let Err(_) = match maybe_entry {
                                    Some(entry) => {
                                        let maddrs = entry.node.value.iter().cloned().collect();
                                        self.behaviour_mut().rr.send_response(
                                            channel,
                                            Response::Found {
                                                peer: request.peer,
                                                maddrs,
                                            },
                                        )
                                    }
                                    None => self.behaviour_mut().rr.send_response(
                                        channel,
                                        Response::NotFound { peer: request.peer },
                                    ),
                                } {
                                    tracing::error!("Failed to send response");
                                }
                            }
                            None => {
                                let maddrs = self.external_addresses().cloned().collect();
                                if let Err(_) = self
                                    .behaviour_mut()
                                    .rr
                                    .send_response(channel, Response::Found { peer, maddrs })
                                {
                                    tracing::error!("Failed to send response");
                                }
                            }
                        }
                        // self.rr_queries.insert(query_id, (request.peer, channel));
                    }
                    request_response::Message::Response { .. } => {
                        tracing::debug!("unhandled rr response: {message:?}")
                    }
                },
                _ => tracing::debug!("unhandled rr event: {event:?}"),
            },
            BehaviourEvent::Identify(event) => {
                match event {
                    identify::Event::Received { peer_id, info, .. } => {
                        tracing::info!("Received identify event with info: {info:?}");

                        for addr in info.listen_addrs.clone() {
                            tracing::info!("Adding address to Kademlia: {addr}");
                            self.behaviour_mut().kad.add_address(&peer_id, addr);
                        }
                    }
                    _ => tracing::debug!("Received unhandled identify event: {event:?}"),
                };
            }
            BehaviourEvent::Kad(event) => self.on_kademlia_event(event),
        }
    }

    fn on_kademlia_event(&mut self, event: kad::Event) {
        match event {
            _ => tracing::trace!("Unhandled Kademlia event: {event:?}"),
        }
    }
}
