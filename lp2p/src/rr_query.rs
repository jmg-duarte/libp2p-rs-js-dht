use std::time::Duration;

use clap::Parser;
use libp2p::{
    core,
    futures::StreamExt,
    identify,
    identity::{self, Keypair},
    kad::{self, GetRecordOk, QueryResult, RecordKey},
    noise,
    request_response::{self, ProtocolSupport},
    swarm::{self, NetworkBehaviour, SwarmEvent},
    tcp, websocket, yamux, Multiaddr, PeerId, StreamProtocol, Swarm, Transport,
};
use lp2p::{extract_peer_id, lp::LpCbor, Request, Response};
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
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::DEBUG))
        .init();

    let app = App::parse();

    let identity = identity::Keypair::generate_ed25519();
    let local_peer_id = identity.public().to_peer_id();
    tracing::info!("Local peer id {}", local_peer_id);
    let swarm = create_swarm(&identity, vec![app.bootnode.clone()]);

    tracing::info!("PeerId bytes: {:?}", &app.query.to_bytes());

    let cancellation_token = CancellationToken::new();

    let mut state = State {
        swarm,
        cancel: cancellation_token,
        query: app.query,
    };

    state.swarm.dial(app.bootnode.clone()).unwrap();

    loop {
        tokio::select! {
            event = state.swarm.select_next_some() => state.on_swarm_event(event),
            _ = state.cancel.cancelled() => break,
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
        swarm::Config::with_tokio_executor(),
    )
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    identify: identify::Behaviour,
    rr: request_response::Behaviour<LpCbor<Request, Response>>,
}

impl Behaviour {
    fn new(keypair: Keypair, bootnodes: Vec<Multiaddr>) -> Self {
        let identify = identify::Behaviour::new(identify::Config::new(
            "/polka-test/identify/1.0.0".to_string(),
            keypair.public(),
        ));

        let rr = request_response::Behaviour::new(
            [(StreamProtocol::new("/rr/1.0.0"), ProtocolSupport::Outbound)],
            Default::default(),
        );

        Self { identify, rr }
    }
}

struct State {
    swarm: Swarm<Behaviour>,
    cancel: CancellationToken,
    query: PeerId,
}

impl State {
    fn on_swarm_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::Behaviour(event) => self.on_behaviour_event(event),
            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                num_established,
                concurrent_dial_errors,
                established_in,
            } => {
                self.swarm
                    .behaviour_mut()
                    .rr
                    .send_request(&peer_id, Request { peer: self.query });
            }
            _ => tracing::debug!("Received unhandled event: {event:?}"),
        }
    }

    fn on_behaviour_event(&mut self, event: BehaviourEvent) {
        match event {
            BehaviourEvent::Identify(event) => {
                tracing::debug!("Received unhandled identify event: {event:?}")
            }
            BehaviourEvent::Rr(event) => self.on_rr_event(event),
        }
    }

    #[tracing::instrument(skip_all)]
    fn on_rr_event(&mut self, event: request_response::Event<Request, Response>) {
        match event {
            request_response::Event::Message {
                peer,
                connection_id,
                message,
            } => match message {
                request_response::Message::Request {
                    request_id,
                    request,
                    channel,
                } => tracing::info!("Received request: {request:?}"),
                request_response::Message::Response {
                    request_id,
                    response,
                } => {
                    tracing::info!("Received response: {response:?}");
                    self.cancel.cancel();
                }
            },
            _ => tracing::debug!("Unhandled request-response event: {event:?}"),
        }
    }
}
