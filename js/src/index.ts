import { noise } from "@chainsafe/libp2p-noise"
import { yamux } from "@chainsafe/libp2p-yamux"
import { bootstrap } from "@libp2p/bootstrap"
import { ping } from "@libp2p/ping"
import { identify, identifyPush } from "@libp2p/identify"
import { kadDHT, removePublicAddressesMapper } from "@libp2p/kad-dht"
import { peerIdFromString } from "@libp2p/peer-id"
import { webSockets } from "@libp2p/websockets"
import { createLibp2p } from "libp2p"

async function createNode(bootnodes: string[]) {
    return await createLibp2p(
        {
            addresses: {
                listen: [
                    "/ip4/0.0.0.0/tcp/0/ws"
                ]
            },
            transports: [webSockets()],
            connectionEncrypters: [noise()],
            streamMuxers: [yamux()],
            peerDiscovery: [
                bootstrap({
                    list: bootnodes,
                }),
            ],
            services: {
                identify: identify(),
                identifyPush: identifyPush(),
                dht: kadDHT({
                    peerInfoMapper: removePublicAddressesMapper,
                    clientMode: true,
                }),
                ping: ping()
            },
            connectionGater: {
              denyDialMultiaddr: () => false
            },
        },
    )
}

async function main() {
    const [_node, _script, _, bootnodes, query] = process.argv
    if (!bootnodes) {
        throw new Error("Missing \"bootnodes\"")
    }
    if (!query) {
        throw new Error("Missing \"query\"")
    }

    console.log('bootnodes', bootnodes)
    console.log('query', query)

    const node = await createNode(bootnodes.split(","))
    // console.log('node', node)
    // node.addEventListener("self:peer:update", evt => {
    //   console.log('self:peer:update', evt.detail)
    //   console.log(node.getMultiaddrs())
    // })

    const peerId = peerIdFromString(query)
    console.log("contentRouting get", await node.contentRouting.get(peerId.toMultihash().bytes))
}

main()
