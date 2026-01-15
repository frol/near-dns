# NEAR DNS - Agent Guide

This document provides context for AI agents working on the NEAR DNS codebase.

## Project Overview

NEAR DNS is a decentralized DNS system that resolves blockchain-based domain names by querying smart contracts on NEAR Protocol. It consists of two main components:

1. **DNS Server** (Rust) - Handles DNS queries, routes NEAR domains to blockchain, forwards others upstream
2. **DNS Contract** (NEAR/Rust) - Stores DNS records on-chain with owner-based access control

## Repository Structure

```
/mnt/near-dns/
├── Cargo.toml                    # Workspace config (excludes dns-contract)
├── dns-server/                   # DNS server implementation
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # CLI entry, server setup
│       ├── records.rs            # DnsRecord struct, conversion to hickory RData
│       ├── cache.rs              # Moka-based caching layer
│       ├── authority/
│       │   ├── mod.rs
│       │   └── blockchain.rs     # Hickory Authority trait impl
│       └── resolver/
│           ├── mod.rs
│           ├── near.rs           # NEAR RPC resolution logic
│           └── upstream.rs       # Google/Cloudflare DNS forwarding
├── dns-contract/                 # Smart contract (standalone, not in workspace)
│   ├── Cargo.toml
│   └── src/lib.rs                # Contract with DNS record storage
├── README.md
├── CONTRIBUTING.md
├── AGENT.md                      # This file
├── LICENSE-MIT
└── LICENSE-APACHE
```

## Key Technical Details

### DNS Server

**Dependencies:**
- `hickory-server` 0.25 - DNS server framework
- `hickory-proto` 0.25 - DNS protocol types
- `hickory-resolver` 0.25 - Upstream DNS resolution
- `near-api` 0.8 - NEAR RPC client
- `tokio` - Async runtime
- `moka` - Caching
- `clap` - CLI parsing

**Resolution Flow:**
1. Query comes in via UDP/TCP on configured port
2. `BlockchainAuthority::lookup()` extracts TLD
3. If TLD is in whitelist (`near`, `testnet`, etc.), route to `NearResolver`
4. Otherwise, forward to `UpstreamResolver`

**NEAR Resolution Logic (`resolver/near.rs`):**
- For `sub.example.testnet`, tries in order:
  1. `dns.sub.example.testnet` with name `@`
  2. `dns.example.testnet` with name `sub`
  3. `dns.example.testnet` with name `*` (wildcard)

**TLD Whitelist:**
```rust
const KNOWN_NEAR_TLDS: &[&str] = &[
    "near", "testnet", "aurora", "tg", "sweat", "kaiching", "sharddog"
];
```

### DNS Contract

**Storage:** Uses `IterableMap<String, Vec<DnsRecord>>` with keys like `"@:A"`, `"www:CNAME"`

**Owner Model:** Contract deployed as `dns.<account>.<tld>` is owned by `<account>.<tld>`

**Key Methods:**
- `dns_query(name, record_type)` - View, returns `Option<Vec<DnsRecord>>`
- `dns_add(name, record)` - Change, appends record
- `dns_update(name, records)` - Change, replaces all records of type
- `dns_delete(name, record_type)` - Change, removes records
- `dns_list_all()` - View, returns all records (enabled by IterableMap)

**Building Contract:**
```bash
cd dns-contract
rustup override set 1.86  # If needed for WASM compatibility
cargo near build non-reproducible-wasm
# Output: target/near/dns_contract.wasm
```

## Deployed Testnet Resources

| Resource | Value |
|----------|-------|
| Main Account | `near-dns.testnet` |
| Contract Account | `dns.near-dns.testnet` |
| Contract Owner | `near-dns.testnet` |
| Legacy Test Account | `neardns1768515956.testnet` |
| Legacy Contract | `dns.neardns1768515956.testnet` |

Credentials are stored in `/home/node/.near-credentials/testnet/`

## Common Tasks

### Running the DNS Server

```bash
cd /mnt/near-dns
RUST_LOG=info cargo run --package near-dns-server -- \
  --bind 127.0.0.1:5353 \
  --rpc-url https://rpc.testnet.near.org
```

### Testing with dig

```bash
dig @127.0.0.1 -p 5353 near-dns.testnet A
dig @127.0.0.1 -p 5353 near-dns.testnet TXT
dig @127.0.0.1 -p 5353 www.near-dns.testnet A
dig @127.0.0.1 -p 5353 google.com A  # Upstream forwarding
```

### Adding DNS Records

```bash
near contract call-function as-transaction dns.near-dns.testnet dns_add \
  json-args '{"name": "@", "record": {"record_type": "A", "value": "1.2.3.4", "ttl": 300, "priority": null}}' \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' \
  sign-as near-dns.testnet network-config testnet sign-with-keychain send
```

### Querying Contract Directly

```bash
near contract call-function as-read-only dns.near-dns.testnet dns_list_all \
  json-args '{}' network-config testnet now
```

### Deploying Contract Updates

```bash
cd dns-contract
cargo near build non-reproducible-wasm

near contract deploy dns.near-dns.testnet \
  use-file target/near/dns_contract.wasm \
  without-init-call \
  network-config testnet sign-with-keychain send
```

## Code Patterns

### Contract Serialization

Use the modern NEAR SDK macro:
```rust
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct DnsRecord {
    pub record_type: String,
    pub value: String,
    pub ttl: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
}
```

### Contract State

```rust
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct DnsContract {
    records: IterableMap<String, Vec<DnsRecord>>,
    owner: AccountId,
}
```

### Server Record Conversion

The `records.rs` file handles converting between contract `DnsRecord` and hickory `RData`:
```rust
impl DnsRecord {
    pub fn to_dns_record(&self, name: &Name, origin: &Name) -> Result<Record, RecordConversionError>
}
```

## Known Issues / LSP Warnings

The LSP may show errors in `dns-server` files that don't affect compilation. These are typically due to:
- LSP not recognizing the full hickory API
- Async trait bounds
- The actual build with `cargo build` succeeds despite LSP errors

## Testing

```bash
# Contract tests
cd dns-contract && cargo test

# Server tests (when implemented)
cd dns-server && cargo test

# Manual integration test
# Terminal 1: Start server
# Terminal 2: Run dig commands
```

## Environment Notes

- NEAR credentials: `/home/node/.near-credentials/testnet/`
- No sudo access in this environment
- Rust toolchain managed via rustup
- Contract requires Rust 1.86 for WASM builds (set with `rustup override`)
