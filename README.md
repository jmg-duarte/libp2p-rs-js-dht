## Setup

This repo supports Nix, to get started you can simply run:
```bash
nix develop
```

And you should be dropped into a `bash` shell with the necessary tools required to build the project.

## Rust

1. Boot server:
   ```bash
   cargo run --release --bin server -- -l "{{listen-addrs}}"
   ```

2. Second server (bootnode multiaddress MUST include `/p2p/<peer-id>`):
   ```bash
   cargo run --release --bin server -- -l "{{listen-addrs}}" -b "{{bootnode-addrs}}"
   ```

3. Execute a query:
   ```bash
   cargo run --release --bin query -- "{{bootnode-addr}}" "{{query-peer-id}}"
   ```

> [!NOTE]
> The Rust client (the `query` binary) supports both TCP and WebSockets!

## Rust/JS

1. Boot server:
   ```bash
   cargo run --release --bin server -- -l "{{listen-addrs}}"
   ```

2. Second server (bootnode multiaddress MUST include `/p2p/<peer-id>`):
   ```bash
   cargo run --release --bin server -- -l "{{listen-addrs}}" -b "{{bootnode-addrs}}"
   ```

3. Client:
   ```bash
   pnpm run run -- "{{bootnode-addrs}}" "{{query-peer-id}}"
   ```

> [!WARNING]
> The JS client is configured to only support WebSockets since the target environment (browser) does not support TCP.

## Docker

There's a Docker container for the server, useful to test network isolation, etc.
You can build it with the following command:
```bash
docker build -f docker/Dockerfile.server --tag "lp2p-server:latest" -D .
```
