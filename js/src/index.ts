import { noise } from "@chainsafe/libp2p-noise"
import { yamux } from "@chainsafe/libp2p-yamux"
import { bootstrap } from "@libp2p/bootstrap"
import { identify } from "@libp2p/identify"
import { kadDHT } from "@libp2p/kad-dht"
import { peerIdFromString } from "@libp2p/peer-id"
import { webSockets } from "@libp2p/websockets"
import { createLibp2p } from "libp2p"
import all from "it-all"
import { multiaddr } from "@multiformats/multiaddr"

async function createNode(bootnode: string) {
    return await createLibp2p(
        {
            connectionEncrypters: [noise()],
            streamMuxers: [yamux()],
            transports: [webSockets()],
            peerDiscovery: [
                bootstrap({
                    list: [bootnode]
                })
            ],
            services: {
                identify: identify(),
                dht: kadDHT({
                    clientMode: true
                })
            },
            connectionMonitor: {
                "enabled": false
            }
        }
    )
}

async function main() {
    const [_node, _script, bootnode, query] = process.argv
    if (!bootnode) {
        throw new Error("Missing \"bootnode\"")
    }
    if (!query) {
        throw new Error("Missing \"query\"")
    }

    console.log(multiaddr(bootnode));

    const node = await createNode(bootnode)
    console.log("local peer id", node.peerId)

    const peerId = peerIdFromString(query)
    const peerIdQuery = peerId.toMultihash().bytes

    const queryResult = node.services.dht.get(peerIdQuery)
    const consumedResult = await all(queryResult)

    console.log(consumedResult)
}

main()
