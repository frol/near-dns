# NEAR DNS

A decentralized DNS system that resolves blockchain-based domain names by querying smart contracts on NEAR Protocol.

## Overview

NEAR DNS enables domain name resolution for NEAR ecosystem TLDs (`.near`, `.tg`, `.testnet`, etc.) by storing DNS records in smart contracts. Each NEAR account can deploy a DNS contract as a subaccount (`dns.<account>.<tld>`) to manage their domain's DNS records.

### How It Works

1. **DNS Query**: Client queries `example.near` A record
2. **TLD Detection**: Server identifies `.near` as a NEAR TLD
3. **Contract Lookup**: Server queries `dns.example.near` contract for records
4. **Response**: DNS records are returned from the blockchain

For traditional domains (`.com`, `.org`, etc.), queries are forwarded to upstream DNS servers (Google/Cloudflare).

## Architecture

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  DNS Client │────▶│  NEAR DNS Server │────▶│  NEAR Blockchain│
│   (dig)     │◀────│   (Rust/Hickory) │◀────│  (DNS Contract) │
└─────────────┘     └──────────────────┘     └─────────────────┘
                            │
                            ▼
                    ┌───────────────┐
                    │ Upstream DNS  │
                    │ (Google/CF)   │
                    └───────────────┘
```

## Components

### DNS Server (`dns-server/`)

A Rust DNS server built with [Hickory DNS](https://github.com/hickory-dns/hickory-dns) that:

- Resolves NEAR domains by querying smart contracts via RPC
- Forwards non-NEAR domains to upstream DNS servers
- Supports hierarchical subdomain resolution
- Implements wildcard record matching
- Caches responses for performance

### DNS Contract (`dns-contract/`)

A NEAR smart contract that stores DNS records with:

- Support for common record types (A, AAAA, CNAME, MX, TXT, etc.)
- Owner-only record management (parent account controls records)
- Wildcard record support (`*` entries)
- Iterable storage for listing all records

## Quick Start

### Prerequisites

- Rust 1.70+
- NEAR CLI (`npm install -g near-cli` or `cargo install near-cli-rs`)
- A NEAR testnet account

### Running the DNS Server

```bash
# Clone and build
cd dns-server
cargo build --release

# Run the server (testnet)
RUST_LOG=info cargo run --release -- \
  --bind 127.0.0.1:5353 \
  --rpc-url https://rpc.testnet.near.org

# Test with dig
dig @127.0.0.1 -p 5353 near-dns.testnet A
dig @127.0.0.1 -p 5353 near-dns.testnet TXT
dig @127.0.0.1 -p 5353 google.com A  # Forwarded upstream
```

### Deploying Your Own DNS Contract

```bash
# Create a subaccount for DNS
near account create-account fund-myself dns.youraccount.testnet '0.5 NEAR' \
  autogenerate-new-keypair save-to-keychain \
  sign-as youraccount.testnet network-config testnet sign-with-keychain send

# Build the contract
cd dns-contract
cargo near build non-reproducible-wasm

# Deploy with initialization
near contract deploy dns.youraccount.testnet \
  use-file target/near/dns_contract.wasm \
  with-init-call new json-args '{}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  network-config testnet sign-with-keychain send
```

### Managing DNS Records

```bash
# Add an A record
near contract call-function as-transaction dns.youraccount.testnet dns_add \
  json-args '{"name": "@", "record": {"record_type": "A", "value": "1.2.3.4", "ttl": 300, "priority": null}}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  sign-as youraccount.testnet network-config testnet sign-with-keychain send

# Add a subdomain
near contract call-function as-transaction dns.youraccount.testnet dns_add \
  json-args '{"name": "www", "record": {"record_type": "A", "value": "1.2.3.5", "ttl": 300, "priority": null}}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  sign-as youraccount.testnet network-config testnet sign-with-keychain send

# Add a wildcard record (matches any subdomain)
near contract call-function as-transaction dns.youraccount.testnet dns_add \
  json-args '{"name": "*", "record": {"record_type": "A", "value": "1.2.3.100", "ttl": 300, "priority": null}}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  sign-as youraccount.testnet network-config testnet sign-with-keychain send

# Query records
near contract call-function as-read-only dns.youraccount.testnet dns_query \
  json-args '{"name": "@", "record_type": "A"}' \
  network-config testnet now

# List all records
near contract call-function as-read-only dns.youraccount.testnet dns_list_all \
  json-args '{}' network-config testnet now
```

## Contract API

### View Methods

| Method | Arguments | Description |
|--------|-----------|-------------|
| `dns_query` | `name: String, record_type: String` | Query specific record type for a name |
| `dns_query_all` | `name: String` | Get all record types for a name |
| `dns_list_names` | - | List all DNS names with records |
| `dns_list_all` | - | List all records in the contract |
| `get_owner` | - | Get the contract owner |

### Change Methods

| Method | Arguments | Description |
|--------|-----------|-------------|
| `dns_add` | `name: String, record: DnsRecord` | Add a DNS record |
| `dns_update` | `name: String, records: Vec<DnsRecord>` | Replace all records of a type |
| `dns_delete` | `name: String, record_type: Option<String>` | Delete records |
| `transfer_ownership` | `new_owner: AccountId` | Transfer contract ownership |

### DnsRecord Structure

```json
{
  "record_type": "A",
  "value": "192.168.1.1",
  "ttl": 300,
  "priority": null
}
```

Supported record types: `A`, `AAAA`, `CNAME`, `MX`, `TXT`, `NS`, `SRV`, `SOA`, `PTR`, `CAA`

## Resolution Logic

For a query like `sub.example.testnet`:

1. Check if `testnet` is a known NEAR TLD
2. Try `dns.sub.example.testnet` with name `@`
3. Try `dns.example.testnet` with name `sub`
4. Try `dns.example.testnet` with name `*` (wildcard)
5. Return NXDOMAIN if no records found

## Supported TLDs

The DNS server recognizes these NEAR TLDs:
- `near` (mainnet)
- `testnet`
- `aurora`
- `tg`
- `sweat`
- `kaiching`
- `sharddog`

All other TLDs are forwarded to upstream DNS servers.

## Deployed Contracts

### Testnet

- **Contract**: `dns.near-dns.testnet`
- **Owner**: `near-dns.testnet`

## Development

```bash
# Run DNS server tests
cd dns-server
cargo test

# Run contract tests
cd dns-contract
cargo test

# Build contract WASM
cd dns-contract
cargo near build non-reproducible-wasm
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
