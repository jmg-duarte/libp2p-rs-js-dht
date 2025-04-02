use std::time::Duration;

use clap::Parser;
use libp2p::{
    core,
    futures::StreamExt,
    identify,
    identity::{self, Keypair},
    noise,
    swarm::{self, NetworkBehaviour, SwarmEvent},
    tcp, websocket, yamux, Multiaddr, Swarm, Transport,
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[derive(Clone, Debug, clap::Parser)]
struct App {
    bootnode: Multiaddr,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::DEBUG))
        .init();

    let app = App::parse();

    let identity = identity::Keypair::generate_ed25519();
    let local_peer_id = identity.public().to_peer_id();
    tracing::info!("Local peer id {}", local_peer_id);
    let mut swarm = create_swarm(&identity);

    // let ifaces = [/*"/ip4/0.0.0.0/tcp/0", */ "/ip4/0.0.0.0/tcp/0/ws"];
    // ifaces
    //     .into_iter()
    //     .map(str::parse::<Multiaddr>)
    //     .map(Result::unwrap)
    //     .for_each(|maddr| {
    //         swarm.listen_on(maddr).unwrap();
    //     });

    swarm.dial(app.bootnode).unwrap();

    let mut state = State { swarm };

    loop {
        tokio::select! {
            event = state.swarm.select_next_some() => state.on_swarm_event(event)
        }
    }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    identify: identify::Behaviour,
}

impl Behaviour {
    fn new(keypair: Keypair) -> Self {
        let identify = identify::Behaviour::new(identify::Config::new(
            "/polka-test/identify/1.0.0".to_string(),
            keypair.public(),
        ));

        Self { identify }
    }
}

struct State {
    swarm: Swarm<Behaviour>,
}

impl State {
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
            SwarmEvent::Behaviour(event) => self.on_behaviour_event(event),
            _ => tracing::debug!("Received unhandled event: {event:?}"),
        }
    }

    fn on_behaviour_event(&mut self, event: BehaviourEvent) {
        match event {
            BehaviourEvent::Identify(event) => {
                tracing::debug!("Received unhandled identify event: {event:?}")
            }
        }
    }
}

fn create_swarm(identity: &Keypair) -> Swarm<Behaviour> {
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

    Swarm::new(
        tcp_ws_transport,
        Behaviour::new(identity.to_owned()),
        local_peer_id,
        swarm::Config::with_tokio_executor().with_idle_connection_timeout(Duration::from_secs(10)),
    )
}
