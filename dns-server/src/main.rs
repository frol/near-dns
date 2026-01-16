mod authority;
mod cache;
mod records;
mod resolver;

use authority::BlockchainAuthority;
use cache::DnsCache;
use resolver::near::NearResolver;
use resolver::upstream::UpstreamResolver;

use clap::Parser;
use hickory_server::authority::{AuthorityObject, Catalog};
use hickory_server::ServerFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, UdpSocket};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// NEAR DNS Server - Resolve .near and other blockchain TLDs via NEAR Protocol
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Address to bind the DNS server to
    #[arg(short, long, default_value = "127.0.0.1:5355")]
    bind: SocketAddr,

    /// NEAR RPC URL
    #[arg(short, long, default_value = "https://rpc.testnet.near.org")]
    rpc_url: String,

    /// Enable TCP support
    #[arg(long, default_value = "true")]
    tcp: bool,

    /// TCP connection timeout in seconds
    #[arg(long, default_value = "30")]
    tcp_timeout: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    info!("Starting NEAR DNS Server");
    info!("Bind address: {}", args.bind);
    info!("NEAR RPC URL: {}", args.rpc_url);

    // Create the cache
    let cache = DnsCache::new();

    // Create the NEAR resolver
    let near_resolver = NearResolver::new(&args.rpc_url, cache)
        .map_err(|e| format!("Failed to create NEAR resolver: {}", e))?;

    // Create the upstream resolver
    let upstream_resolver = UpstreamResolver::new();

    // Create the blockchain authority
    let authority = BlockchainAuthority::new(near_resolver, upstream_resolver);

    // Create a catalog and add our authority for the root zone
    let mut catalog = Catalog::new();
    
    // Register the authority for all queries (root zone)
    let authority: Arc<dyn AuthorityObject> = Arc::new(authority);
    catalog.upsert(
        hickory_proto::rr::LowerName::from(hickory_proto::rr::Name::root()),
        vec![authority],
    );

    // Create the server
    let mut server = ServerFuture::new(catalog);

    // Bind UDP socket
    let udp_socket = UdpSocket::bind(args.bind).await?;
    info!("UDP socket bound to {}", args.bind);
    server.register_socket(udp_socket);

    // Optionally bind TCP listener
    if args.tcp {
        let tcp_listener = TcpListener::bind(args.bind).await?;
        info!("TCP listener bound to {}", args.bind);
        server.register_listener(tcp_listener, Duration::from_secs(args.tcp_timeout));
    }

    info!("DNS server is running. Press Ctrl+C to stop.");
    info!("Test with: dig @{} <domain> A", args.bind);

    // Run the server
    match server.block_until_done().await {
        Ok(_) => {
            info!("Server shutdown gracefully");
            Ok(())
        }
        Err(e) => {
            error!("Server error: {}", e);
            Err(e.into())
        }
    }
}
