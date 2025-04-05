use std::time::Duration;

use clap::Parser;
use libp2p::{
    core,
    futures::StreamExt,
    identify,
    identity::{self, Keypair},
    kad::{self, GetRecordOk, QueryResult, RecordKey},
    noise,
    swarm::{self, NetworkBehaviour, SwarmEvent},
    tcp, websocket, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use lp2p::extract_peer_id;
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[derive(Debug, Clone, clap::Parser)]
struct App {
    bootnode: Multiaddr,
    query: PeerId,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
        .init();

    let app = App::parse();

    let identity = identity::Keypair::generate_ed25519();
    let local_peer_id = identity.public().to_peer_id();
    tracing::info!("Local peer id {}", local_peer_id);
    let mut swarm = create_swarm(&identity, vec![app.bootnode]);

    tracing::info!("PeerId bytes: {:?}", &app.query.to_bytes());

    swarm
        .behaviour_mut()
        .kad
        .get_record(RecordKey::new(&app.query.to_bytes()));

    let mut state = State { swarm };

    let cancellation_token = CancellationToken::new();

    loop {
        tokio::select! {
            event = state.swarm.select_next_some() => state.on_swarm_event(event, cancellation_token.child_token()),
            _ = cancellation_token.cancelled() => break,
        }
    }
}

fn create_swarm(identity: &Keypair, bootnodes: Vec<Multiaddr>) -> Swarm<Behaviour> {
    let local_peer_id = identity.public().to_peer_id();
    tracing::info!("Local peer id: {local_peer_id}");

    let noise_config = noise::Config::new(&identity).unwrap(); // TODO: proper error handling
    let muxer_config = yamux::Config::default();

    let tcp_config = tcp::Config::new();
    let tcp_transport = tcp::tokio::Transport::new(tcp_config.clone());

    let ws = websocket::WsConfig::new(tcp::tokio::Transport::new(tcp_config));

    Swarm::new(
        ws.or_transport(tcp_transport)
            .upgrade(core::upgrade::Version::V1Lazy)
            .authenticate(noise_config)
            .multiplex(muxer_config)
            .boxed(),
        Behaviour::new(identity.to_owned(), bootnodes),
        local_peer_id,
        swarm::Config::with_tokio_executor().with_idle_connection_timeout(Duration::from_secs(10)),
    )
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    identify: identify::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
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
        kad.set_mode(Some(kad::Mode::Client));
        for maddr in bootnodes {
            let peer = extract_peer_id(&maddr).unwrap();
            kad.add_address(&peer, maddr);
        }
        if kad.bootstrap().is_err() {
            tracing::warn!("No peers were added");
        }

        Self { identify, kad }
    }
}

struct State {
    swarm: Swarm<Behaviour>,
}

impl State {
    fn on_swarm_event(
        &mut self,
        event: SwarmEvent<BehaviourEvent>,
        cancellation_token: CancellationToken,
    ) {
        match event {
            SwarmEvent::Behaviour(event) => self.on_behaviour_event(event, cancellation_token),
            _ => tracing::debug!("Received unhandled event: {event:?}"),
        }
    }

    fn on_behaviour_event(&mut self, event: BehaviourEvent, cancellation_token: CancellationToken) {
        match event {
            BehaviourEvent::Identify(event) => {
                tracing::debug!("Received unhandled identify event: {event:?}")
            }
            BehaviourEvent::Kad(event) => match event {
                kad::Event::OutboundQueryProgressed { result, .. } => match result {
                    QueryResult::GetRecord(get_record_ok) => {
                        match get_record_ok {
                            Ok(ok) => {
                                if let GetRecordOk::FoundRecord(record) = ok {
                                    let peer_id =
                                        PeerId::from_bytes(&record.record.key.to_vec()).unwrap();
                                    let maddrs: Vec<Multiaddr> =
                                        cbor4ii::serde::from_slice(&record.record.value).unwrap();
                                    tracing::info!("GetRecord returned the following record: {peer_id}::{maddrs:?}");
                                }
                            }
                            Err(err) => {
                                tracing::error!("GetRecord failed with error: {err}");
                            }
                        }
                        cancellation_token.cancel();
                    }
                    QueryResult::GetClosestPeers(peers) => match peers {
                        Ok(peers) => {
                            tracing::info!("Received peers: {peers:?}");
                        }
                        Err(err) => {
                            tracing::error!("Failed to get closest peers with error: {err}")
                        }
                    },
                    _ => tracing::debug!(
                        "Received unhandled outbound query progress event: {result:?}"
                    ),
                },
                _ => tracing::debug!("Received unhandled kademlia event: {event:?}"),
            },
        }
    }
}
