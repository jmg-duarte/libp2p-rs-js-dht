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
