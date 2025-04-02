#![cfg(target_arch = "wasm32")]

use std::str::FromStr;

use libp2p::{
    core,
    futures::StreamExt,
    identify,
    identity::{self, Keypair},
    kad::{self, GetRecordOk, GetRecordResult, QueryResult, RecordKey},
    noise, ping,
    swarm::{self, NetworkBehaviour, SwarmEvent},
    websocket_websys as websocket, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    fmt::time::UtcTime, layer::SubscriberExt, util::SubscriberInitExt, Layer,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn setup_logging() {
    console_error_panic_hook::set_once();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_timer(UtcTime::rfc_3339()) // std::time is not available in browsers
        .with_writer(tracing_web::MakeConsoleWriter) // write events to the console
        .with_filter(LevelFilter::DEBUG);

    let _ = tracing_subscriber::registry().with(fmt_layer).try_init();
}

#[wasm_bindgen]
pub async fn perform_query(bootnodes: Vec<String>, query: String) -> Result<String, String> {
    let bootnodes = bootnodes
        .into_iter()
        .map(|s| Multiaddr::from_str(&s))
        .collect::<Result<Vec<Multiaddr>, _>>()
        .map_err(|err| err.to_string())?;

    let query = PeerId::from_str(&query).map_err(|err| err.to_string())?;

    tracing::info!("Query: {}", query);

    perform_query_inner(bootnodes, query)
        .await
        .map(|maddrs| maddrs.iter().map(ToString::to_string).collect())
}

async fn perform_query_inner(
    bootnodes: Vec<Multiaddr>,
    query: PeerId,
) -> Result<Vec<Multiaddr>, String> {
    // This node is ephemeral so we don't care for the actual identity
    // we can read it from the user selected account but to query the DHT it doesn't make a difference
    let identity = identity::Keypair::generate_ed25519();

    let swarm = inner_create_swarm(&identity, bootnodes);
    let mut state = State { swarm };

    state.event_loop(query).await
}

fn inner_create_swarm(identity: &Keypair, bootnodes: Vec<Multiaddr>) -> Swarm<Behaviour> {
    let local_peer_id = identity.public().to_peer_id();
    tracing::info!("Local peer id: {local_peer_id}");

    let noise_config = noise::Config::new(&identity).unwrap(); // TODO: proper error handling
    let muxer_config = yamux::Config::default();

    let mut swarm = Swarm::new(
        websocket::Transport::default()
            .upgrade(core::upgrade::Version::V1Lazy)
            .authenticate(noise_config)
            .multiplex(muxer_config)
            .boxed(),
        Behaviour::new(identity.to_owned(), bootnodes.clone()),
        local_peer_id,
        swarm::Config::with_wasm_executor(),
    );

    for node in bootnodes {
        swarm.dial(node).expect("Should be able to dial node");
    }

    swarm
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
}

impl Behaviour {
    fn new(keypair: Keypair, bootnodes: Vec<Multiaddr>) -> Self {
        let ping = ping::Behaviour::new(ping::Config::default());

        let identify = identify::Behaviour::new(identify::Config::new(
            "/polka-test/identify/1.0.0".to_string(),
            keypair.public(),
        ));

        let local_peer_id = keypair.public().to_peer_id();
        let mut kad =
            kad::Behaviour::new(local_peer_id, kad::store::MemoryStore::new(local_peer_id));
        kad.set_mode(Some(kad::Mode::Client));
        for maddr in bootnodes {
            tracing::debug!("Adding multiaddress: {:?}", maddr);

            let peer = match maddr.iter().last() {
                Some(core::multiaddr::Protocol::P2p(peer_id)) => Some(peer_id),
                _ => None,
            }
            .expect("multiaddress should contain a /p2p segment");

            kad.add_address(&peer, maddr);
        }
        Self {
            ping,
            identify,
            kad,
        }
    }
}

struct State {
    swarm: Swarm<Behaviour>,
}

impl State {
    async fn event_loop(&mut self, query: PeerId) -> Result<Vec<Multiaddr>, String> {
        let key = RecordKey::new(&query.to_bytes());
        // Once again, since this is supposed to be ephemeral, we're not storing the query id
        // as it isn't the case (at the time of writing) that multiple in-flight queries should happen
        let query_id = self.swarm.behaviour_mut().kad.get_record(key);
        tracing::debug!("Sent GetRecord request: {query_id:?}");

        loop {
            let event = self.swarm.select_next_some().await;
            match self.on_swarm_event(event) {
                Some(result) => return result,
                None => continue,
            }
        }
    }

    fn on_swarm_event(
        &mut self,
        event: SwarmEvent<BehaviourEvent>,
    ) -> Option<Result<Vec<Multiaddr>, String>> {
        match event {
            SwarmEvent::Behaviour(event) => self.on_behaviour_event(event),
            _ => {
                tracing::debug!("Received unhandled event: {event:?}");
                None
            }
        }
    }

    fn on_behaviour_event(
        &mut self,
        event: BehaviourEvent,
    ) -> Option<Result<Vec<Multiaddr>, String>> {
        match event {
            BehaviourEvent::Kad(event) => match event {
                kad::Event::OutboundQueryProgressed { result, .. } => match result {
                    QueryResult::GetRecord(get_record_ok) => self.on_get_record(get_record_ok),
                    _ => {
                        tracing::debug!(
                            "Received unhandled outbound query progress event: {result:?}"
                        );
                        None
                    }
                },
                _ => {
                    tracing::debug!("Received unhandled kademlia event: {event:?}");
                    None
                }
            },
            _ => {
                tracing::debug!("Received unhandled behaviour event: {event:?}");
                None
            }
        }
    }

    fn on_get_record(
        &mut self,
        get_record: GetRecordResult,
    ) -> Option<Result<Vec<Multiaddr>, String>> {
        match get_record {
            Ok(ok) => match ok {
                GetRecordOk::FoundRecord(record) => {
                    let peer_id = PeerId::from_bytes(&record.record.key.to_vec()).unwrap();
                    let maddrs: Vec<Multiaddr> =
                        cbor4ii::serde::from_slice(&record.record.value).unwrap();
                    tracing::info!(
                        "GetRecord returned the following record: {peer_id}::{maddrs:?}"
                    );
                    return Some(Ok(maddrs));
                }
                GetRecordOk::FinishedWithNoAdditionalRecord { .. } => {
                    tracing::debug!("Received unhandled {ok:?}");
                    return Some(Ok(vec![]));
                }
            },
            Err(err) => {
                tracing::error!("GetRecord failed with error: {err}");
                return Some(Err(err.to_string()));
            }
        }
    }
}
