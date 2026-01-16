# Build stage
FROM rust:1.86-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY dns-server ./dns-server

# Build the release binary using system OpenSSL (not vendored)
ENV OPENSSL_NO_VENDOR=1
RUN cargo build --release --package near-dns-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd --system --create-home --shell /bin/false neardns

WORKDIR /app

# Copy the built binary
COPY --from=builder /app/target/release/near-dns-server /app/near-dns-server

# Set ownership
RUN chown -R neardns:neardns /app

USER neardns

# Default to binding on all interfaces for container use
ENV RUST_LOG=info

# Expose DNS ports (UDP and TCP)
EXPOSE 53/udp
EXPOSE 53/tcp

ENTRYPOINT ["/app/near-dns-server"]

# Default arguments - bind to all interfaces, use mainnet RPC
CMD ["--bind", "0.0.0.0:53", "--rpc-url", "https://rpc.mainnet.near.org"]
