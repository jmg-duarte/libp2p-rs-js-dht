use std::time::Duration;

use clap::Parser;
use libp2p::{
    core,
    futures::StreamExt,
    identify,
    identity::{self, Keypair},
    kad::{self, InboundRequest, QueryResult, Record},
    noise,
    swarm::{self, NetworkBehaviour, SwarmEvent},
    tcp, websocket, yamux, Multiaddr, Swarm, Transport,
};
use lp2p::extract_peer_id;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[derive(Clone, Debug, clap::Parser)]
struct App {
    #[arg(short='l', value_delimiter=',', num_args=1..)]
    listen_addrs: Vec<Multiaddr>,

    #[arg(short='b', value_delimiter=',', num_args=1..)]
    bootnodes: Vec<Multiaddr>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::DEBUG))
        .init();

    let app = App::parse();

    let mut swarm = create_swarm(app.bootnodes);
    for addr in app.listen_addrs {
        swarm.listen_on(addr).unwrap();
    }

    loop {
        tokio::select! {
            event = swarm.select_next_some() => on_swarm_event(&mut swarm, event)
        }
    }
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
        kad.set_mode(Some(kad::Mode::Server));

        for node in bootnodes {
            tracing::info!("Adding address to Kademlia: {node}");
            kad.add_address(&extract_peer_id(&node).unwrap(), node);
        }

        Self { identify, kad }
    }
}

fn create_swarm(bootnodes: Vec<Multiaddr>) -> Swarm<Behaviour> {
    let identity = identity::Keypair::generate_ed25519();
    let local_peer_id = identity.public().to_peer_id();
    tracing::info!("Local peer id: {local_peer_id}");

    let noise_config = noise::Config::new(&identity).unwrap(); // TODO: proper error handling
    let muxer_config = yamux::Config::default();

    let tcp_config = tcp::Config::new();
    let tcp_transport = tcp::tokio::Transport::new(tcp_config.clone());

    let ws = websocket::WsConfig::new(tcp::tokio::Transport::new(tcp_config));
    let tcp_ws_transport = tcp_transport
        .or_transport(ws)
        .upgrade(core::upgrade::Version::V1Lazy)
        .authenticate(noise_config)
        .multiplex(muxer_config)
        .boxed();

    let local_peer_id = identity.public().to_peer_id();

    Swarm::new(
        tcp_ws_transport,
        Behaviour::new(identity, bootnodes),
        local_peer_id,
        swarm::Config::with_tokio_executor().with_idle_connection_timeout(Duration::from_secs(10)),
    )
}

fn on_swarm_event(swarm: &mut Swarm<Behaviour>, event: SwarmEvent<BehaviourEvent>) {
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
        SwarmEvent::Behaviour(event) => on_behaviour_event(swarm, event),
        _ => tracing::debug!("Received unhandled event: {event:?}"),
    }
}

fn on_behaviour_event(swarm: &mut Swarm<Behaviour>, event: BehaviourEvent) {
    match event {
        BehaviourEvent::Identify(event) => {
            match event {
                identify::Event::Received { peer_id, info, .. } => {
                    tracing::info!("Received identify event with info: {info:?}");

                    let is_kad_capable = info
                        .protocols
                        .iter()
                        .any(|stream_protocol| kad::PROTOCOL_NAME.eq(stream_protocol));

                    if is_kad_capable {
                        for addr in info.listen_addrs.clone() {
                            tracing::info!("Adding address to Kademlia: {addr}");
                            swarm.behaviour_mut().kad.add_address(&peer_id, addr);
                        }
                    } else {
                        tracing::warn!("No {} protocol found, skipping...", kad::PROTOCOL_NAME);
                    }

                    // The following code puts the record in the DHT
                    // without it, there's no way to query for a node's multiaddresses!

                    // tracing::info!("Putting listen addresses for peer: {}", peer_id);
                    // let buffer: Vec<u8> = vec![];
                    // let bytes = cbor4ii::serde::to_vec(buffer, &info.listen_addrs).unwrap();
                    // let record = Record::new(peer_id.to_bytes(), bytes);
                    // swarm
                    //     .behaviour_mut()
                    //     .kad
                    //     .put_record(record, kad::Quorum::One)
                    //     .unwrap();
                }
                _ => tracing::debug!("Received unhandled identify event: {event:?}"),
            };
        }
        BehaviourEvent::Kad(event) => match event {
            kad::Event::OutboundQueryProgressed { result, .. } => on_query_result(result),
            kad::Event::InboundRequest { request } => on_inbound_request(request),
            _ => tracing::debug!("Received unhandled kadmelia event: {event:?}"),
        },
    }
}

fn on_query_result(result: QueryResult) {
    match result {
        kad::QueryResult::GetRecord(get_record_ok) => match get_record_ok {
            Ok(ok) => tracing::info!("Successful GetRecord: {ok:?}"),
            Err(err) => tracing::error!("Failed GetRecord: {err:?}"),
        },
        kad::QueryResult::PutRecord(put_record_ok) => match put_record_ok {
            Ok(ok) => tracing::info!("Successful PutRecord: {ok:?}"),
            Err(err) => tracing::error!("Failed PutRecord: {err:?}"),
        },
        _ => tracing::debug!("Received unhandled QueryResult: {result:?}"),
    }
}

fn on_inbound_request(request: InboundRequest) {
    match request {
        request @ kad::InboundRequest::GetRecord { .. } => {
            tracing::info!("Received GetRecord request: {request:?}")
        }
        request @ kad::InboundRequest::PutRecord { .. } => {
            tracing::info!("Received PutRecord request: {request:?}")
        }
        _ => tracing::debug!("Received unhandled InboundRequest: {request:?}"),
    }
}
