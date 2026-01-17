# NEAR DNS

A decentralized DNS system that resolves blockchain-based domain names by querying smart contracts on NEAR Protocol.

## Overview

NEAR DNS enables domain name resolution for NEAR ecosystem TLDs (`.near`, `.testnet`, etc.) by storing DNS records in smart contracts. Each NEAR account can deploy a DNS contract as a subaccount (`dns.<account>.<tld>`) to manage their domain's DNS records.

**Key Design Principle**: NEAR DNS is not intended to be a centralized DNS provider. Since all DNS records are stored on the NEAR blockchain, the DNS server itself is essentially a stateless gateway that translates DNS queries into blockchain lookups. **Self-hosting is encouraged** — you can run your own instance and get the exact same results as any other instance, because the source of truth is always the blockchain.

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

## Learn More

* For developers with or without web3 background: [NEAR DNS - DNS records stored on blockchain and served over DNS protocol](https://www.reddit.com/r/rust/comments/1qew4ra/near_dns_dns_records_stored_on_blockchain_and/)
* For developers with web3 background: [NEAR DNS - DNS records stored on NEAR and served over DNS protocol](https://www.reddit.com/r/nearprotocol/comments/1qek7qz/near_dns_dns_records_stored_on_near_and_servered/)

## Public DNS Server

There is currently one publicly available NEAR DNS server connected to **mainnet**:

```
DNS Server: 185.149.40.161 (port 53)
```

Try it out:

```bash
# Query a mainnet domain
dig @185.149.40.161 neardns.near A

# Expected response:
# neardns.near.    1    IN    A    185.149.40.161
```

> **Note**: This public server is provided for convenience, but self-hosting is encouraged. Since NEAR DNS is stateless (all data comes from the blockchain), running your own instance gives you the same results with better privacy and no single point of failure.

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

- Rust 1.86
- [NEAR CLI](https://near.cli.rs)
- A NEAR account (testnet or mainnet)

### Running the DNS Server

#### For Mainnet

```bash
# Clone and build
git clone https://github.com/frol/near-dns
cd near-dns
cargo build --release --package near-dns-server

# Run the server (mainnet)
RUST_LOG=info ./target/release/near-dns-server \
  --bind 127.0.0.1:5355 \
  --rpc-url https://rpc.mainnet.near.org

# Test with dig
dig @127.0.0.1 -p 5355 neardns.near A
```

#### For Testnet

```bash
# Run the server (testnet)
RUST_LOG=info ./target/release/near-dns-server \
  --bind 127.0.0.1:5355 \
  --rpc-url https://rpc.testnet.near.org

# Test with dig
dig @127.0.0.1 -p 5355 near-dns.testnet A
dig @127.0.0.1 -p 5355 near-dns.testnet TXT
```

Non-NEAR domains are forwarded to upstream DNS servers:

```bash
dig @127.0.0.1 -p 5355 google.com A  # Forwarded upstream
```

### Running with Docker

#### Mainnet (Default)

```bash
# Run with mainnet RPC (default)
docker run -d --name near-dns \
  -p 53:53/udp \
  -p 53:53/tcp \
  frolvlad/near-dns

# Test it
dig @localhost neardns.near A
```

#### Testnet

```bash
# Run with testnet RPC
docker run -d --name near-dns-testnet \
  -p 5355:53/udp \
  -p 5355:53/tcp \
  frolvlad/near-dns \
  --bind 0.0.0.0:53 \
  --rpc-url https://rpc.testnet.near.org

# Test it
dig @localhost -p 5355 near-dns.testnet A
```

#### Additional Options

```bash
# Run with custom log level
docker run -d --name near-dns \
  -e RUST_LOG=debug \
  -p 53:53/udp \
  -p 53:53/tcp \
  frolvlad/near-dns

# View logs
docker logs -f near-dns
```

#### Building from Source

```bash
docker build -t near-dns .
docker run -d -p 5355:53/udp -p 5355:53/tcp near-dns
```

### Register Your Own Domain (aka Deploying Your Own DNS Contract)

Build the contract first using [`cargo near`](https://github.com/near/cargo-near):

```bash
cd dns-contract
cargo near build
```

#### Mainnet

```bash
# Create a subaccount for DNS
near account create-account fund-myself dns.youraccount.near '2.1 NEAR' \
  autogenerate-new-keypair save-to-keychain \
  sign-as youraccount.near network-config mainnet sign-with-keychain send

# Build the contract
cd dns-contract
cargo near build non-reproducible-wasm

# Deploy with initialization
near contract deploy dns.youraccount.near \
  use-file target/near/dns_contract.wasm \
  with-init-call new json-args '{}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  network-config mainnet sign-with-keychain send
```

#### Testnet

Unlike mainnet, you can create a new account and get some free NEAR tokens on testnet using this simple command:

```bash
near account create-account sponsor-by-faucet-service
```

```bash
# Create a subaccount for DNS
near account create-account fund-myself dns.youraccount.testnet '2.1 NEAR' \
  autogenerate-new-keypair save-to-keychain \
  sign-as youraccount.testnet network-config testnet sign-with-keychain send

# Deploy with initialization
near contract deploy dns.youraccount.testnet \
  use-file target/near/dns_contract.wasm \
  with-init-call new json-args '{}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  network-config testnet sign-with-keychain send
```

### Managing DNS Records

The examples below use testnet. For mainnet, replace `.testnet` with `.near` and `network-config testnet` with `network-config mainnet`.

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

For a query like `sub.example.near`:

1. Check if `near` is a known NEAR TLD
2. Try `dns.sub.example.near` with name `@`
3. Try `dns.example.near` with name `sub`
4. Try `dns.example.near` with name `*` (wildcard)
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

### Mainnet

- **Contract**: `dns.neardns.near`
- **Owner**: `neardns.near`

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
