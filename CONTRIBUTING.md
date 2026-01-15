# Contributing to NEAR DNS

Thank you for your interest in contributing to NEAR DNS! This document provides guidelines and information for contributors.

## Getting Started

1. Fork the repository
2. Clone your fork locally
3. Set up the development environment (see below)
4. Create a branch for your changes
5. Make your changes and test them
6. Submit a pull request

## Development Setup

### Prerequisites

- **Rust**: 1.70 or later (`rustup update stable`)
- **NEAR CLI**: `cargo install near-cli-rs` or `npm install -g near-cli`
- **wasm32 target**: `rustup target add wasm32-unknown-unknown`
- **cargo-near**: `cargo install cargo-near`

### Building

```bash
# Build the DNS server
cd dns-server
cargo build

# Build the smart contract
cd dns-contract
cargo near build non-reproducible-wasm
```

### Testing

```bash
# Run DNS server tests
cd dns-server
cargo test

# Run contract tests
cd dns-contract
cargo test

# Run all tests from workspace root
cargo test --workspace
```

## Project Structure

```
near-dns/
├── dns-server/          # Rust DNS server
│   ├── src/
│   │   ├── main.rs      # CLI entry point
│   │   ├── authority/   # Hickory DNS authority implementation
│   │   ├── resolver/    # NEAR and upstream resolvers
│   │   ├── cache.rs     # Response caching
│   │   └── records.rs   # DNS record conversion
│   └── Cargo.toml
├── dns-contract/        # NEAR smart contract
│   ├── src/lib.rs       # Contract implementation
│   └── Cargo.toml
├── README.md
├── CONTRIBUTING.md
├── AGENT.md
├── LICENSE-MIT
└── LICENSE-APACHE
```

## Code Style

### Rust

- Follow standard Rust formatting (`cargo fmt`)
- Run clippy and fix warnings (`cargo clippy`)
- Use meaningful variable and function names
- Add documentation comments for public APIs
- Keep functions focused and reasonably sized

### Commit Messages

- Use present tense ("Add feature" not "Added feature")
- Use imperative mood ("Move cursor to..." not "Moves cursor to...")
- Keep the first line under 72 characters
- Reference issues when applicable

Examples:
```
Add wildcard DNS record support

Implement MX record priority handling

Fix upstream DNS forwarding for .com domains

Closes #123
```

## Pull Request Process

1. **Ensure tests pass**: Run `cargo test --workspace` before submitting
2. **Update documentation**: If you change APIs, update the README
3. **Keep PRs focused**: One feature or fix per PR
4. **Describe your changes**: Explain what and why in the PR description
5. **Be responsive**: Address review feedback promptly

## Areas for Contribution

### High Priority

- [ ] DNSSEC support
- [ ] TCP DNS query support improvements
- [ ] Additional record type support (HTTPS, SVCB)
- [ ] Performance optimizations
- [ ] Integration tests

### Good First Issues

- Documentation improvements
- Additional unit tests
- Error message improvements
- Code cleanup and refactoring

### Contract Improvements

- Batch record operations
- Record expiration/TTL enforcement
- Access control lists (delegate record management)
- Migration support for contract upgrades

### Server Improvements

- Metrics and monitoring (Prometheus)
- Configuration file support
- DNS-over-HTTPS (DoH) support
- DNS-over-TLS (DoT) support
- Cluster/replication support

## Testing Guidelines

### Unit Tests

- Test individual functions in isolation
- Use descriptive test names
- Cover edge cases and error conditions

### Integration Tests

- Test the DNS server with real queries
- Test contract deployment and interaction
- Use testnet for end-to-end testing

### Manual Testing

```bash
# Start the DNS server
RUST_LOG=info cargo run --package near-dns-server -- \
  --bind 127.0.0.1:5353 \
  --rpc-url https://rpc.testnet.near.org

# Test queries
dig @127.0.0.1 -p 5353 near-dns.testnet A
dig @127.0.0.1 -p 5353 near-dns.testnet TXT
dig @127.0.0.1 -p 5353 google.com A
```

## Security

If you discover a security vulnerability, please do NOT open a public issue. Instead, email the maintainers directly with details.

## Questions?

- Open a GitHub issue for bugs or feature requests
- Start a discussion for questions or ideas

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (MIT OR Apache-2.0).
