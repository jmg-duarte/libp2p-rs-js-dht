import { noise } from "@chainsafe/libp2p-noise"
import { yamux } from "@chainsafe/libp2p-yamux"
import { autoNAT } from "@libp2p/autonat"
import { bootstrap } from "@libp2p/bootstrap"
import { identify, identifyPush } from "@libp2p/identify"
import { kadDHT } from "@libp2p/kad-dht"
import { peerIdFromString } from "@libp2p/peer-id"
import { webSockets } from "@libp2p/websockets"
import { multiaddr } from "@multiformats/multiaddr"
import { createLibp2p } from "libp2p"
import { perform_query, setup_logging } from "kad-query"
import { tcp } from "@libp2p/tcp"


async function createNode(bootnodes: string[]) {
    return await createLibp2p(
        {
            addresses: {
                listen: [
                    "/ip4/0.0.0.0/tcp/0",
                    "/ip4/0.0.0.0/tcp/0/ws"
                ]
            },
            transports: [tcp(), webSockets()],
            connectionEncrypters: [noise()],
            streamMuxers: [yamux()],
            peerDiscovery: [
                bootstrap({
                    // Add a list of bootnodes to ensure they're around
                    list: bootnodes,
                }),
            ],
            services: {
                identify: identify(),
                identifyPush: identifyPush(),
                dht: kadDHT({
                    // This is supposed to be an ephemeral client
                    clientMode: false,
                    // Required for local testing, by default, js-libp2p will remove them,
                    // even though this isn't documented anywhere...
                    peerInfoMapper: (peer) => peer
                }),
                autonat: autoNAT(),
            },
            connectionMonitor: {
                enabled: false
            },
        },
    )
}

async function main() {
    const [_node, _script, bootnodesArg, query] = process.argv
    if (!bootnodesArg) {
        throw new Error("Missing \"bootnodes\"")
    }
    if (!query) {
        throw new Error("Missing \"query\"")
    }
    const bootnodes = bootnodesArg.split(",")



    const node = await createNode(bootnodes)
    const peerId = peerIdFromString(query)

    for (const bootnode of bootnodes) {
        console.log(`Dialing ${bootnode}`)
        const _ = await node.dial(multiaddr(bootnode))
    }

    console.log(await node.contentRouting.get(peerId.toMultihash().bytes, { signal: AbortSignal.timeout(5000) }))
}

main()
