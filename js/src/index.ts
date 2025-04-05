import { noise } from "@chainsafe/libp2p-noise"
import { yamux } from "@chainsafe/libp2p-yamux"
import { peerIdFromString } from "@libp2p/peer-id"
import { webSockets } from "@libp2p/websockets"
import { multiaddr } from "@multiformats/multiaddr"
import { createLibp2p } from "libp2p"
import { tcp } from "@libp2p/tcp"
import { cborStream } from "it-cbor-stream"


async function createNode(bootnodes: string[]) {
    return await createLibp2p(
        {
            addresses: {},
            connectionEncrypters: [noise()],
            streamMuxers: [yamux()],
            transports: [tcp(), webSockets()],
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

    console.log(bootnodes)
    const conn = await node.dialProtocol(multiaddr(bootnodes[0]), ["/rr/1.0.0"]);
    const lpCbor = cborStream(conn);
    await lpCbor.write({ peer: peerId.toString() })
    console.log(await lpCbor.read())

}

main()
