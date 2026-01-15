use moka::future::Cache;
use std::time::Duration;

use crate::records::DnsRecord;

/// Cache for contract existence checks
pub type ContractCache = Cache<String, bool>;

/// Cache key for DNS records: (contract_id, dns_name, record_type)
pub type RecordCacheKey = (String, String, String);

/// Cache for DNS records
pub type RecordCache = Cache<RecordCacheKey, Vec<DnsRecord>>;

/// Configuration for cache TTLs
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// TTL for contract existence cache (default: 5 minutes)
    pub contract_ttl: Duration,
    /// Default TTL for record cache if not specified by record (default: 5 minutes)
    pub default_record_ttl: Duration,
    /// Maximum entries in each cache
    pub max_entries: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            contract_ttl: Duration::from_secs(300),    // 5 minutes
            default_record_ttl: Duration::from_secs(300), // 5 minutes
            max_entries: 10_000,
        }
    }
}

/// DNS cache manager
#[derive(Clone)]
pub struct DnsCache {
    /// Contract existence cache
    pub contract: ContractCache,
    /// DNS record cache
    pub records: RecordCache,
}

impl DnsCache {
    /// Create a new cache with default configuration
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Create a new cache with custom configuration
    pub fn with_config(config: CacheConfig) -> Self {
        let contract = Cache::builder()
            .time_to_live(config.contract_ttl)
            .max_capacity(config.max_entries)
            .build();

        let records = Cache::builder()
            .time_to_live(config.default_record_ttl)
            .max_capacity(config.max_entries)
            .build();

        Self {
            contract,
            records,
        }
    }

    /// Check if a contract existence is cached
    pub async fn get_contract(&self, contract_id: &str) -> Option<bool> {
        self.contract.get(contract_id).await
    }

    /// Cache a contract existence result
    pub async fn insert_contract(&self, contract_id: String, exists: bool) {
        self.contract.insert(contract_id, exists).await;
    }

    /// Get cached DNS records
    pub async fn get_records(&self, contract_id: &str, dns_name: &str, record_type: &str) -> Option<Vec<DnsRecord>> {
        let key = (
            contract_id.to_string(),
            dns_name.to_string(),
            record_type.to_string(),
        );
        self.records.get(&key).await
    }

    /// Cache DNS records
    pub async fn insert_records(
        &self,
        contract_id: String,
        dns_name: String,
        record_type: String,
        records: Vec<DnsRecord>,
    ) {
        let key = (contract_id, dns_name, record_type);
        self.records.insert(key, records).await;
    }
}

impl Default for DnsCache {
    fn default() -> Self {
        Self::new()
    }
}
