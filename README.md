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
