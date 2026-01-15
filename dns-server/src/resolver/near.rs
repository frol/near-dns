use near_api::{Account, Contract, NetworkConfig};
use serde_json::json;
use std::str::FromStr;
use tracing::{debug, info, warn};
use url::Url;

use crate::cache::DnsCache;
use crate::records::DnsRecord;

/// Error type for NEAR resolution
#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error("Invalid domain format")]
    InvalidDomain,
    #[error("Not a NEAR TLD")]
    NotNearTld,
    #[error("Domain not found (NXDOMAIN)")]
    NotFound,
    #[error("RPC error: {0}")]
    RpcError(String),
    #[error("Invalid account ID: {0}")]
    InvalidAccountId(String),
}

/// NEAR blockchain DNS resolver
pub struct NearResolver {
    network: NetworkConfig,
    cache: DnsCache,
}

impl NearResolver {
    /// Create a new NEAR resolver with the given RPC URL
    pub fn new(rpc_url: &str, cache: DnsCache) -> Result<Self, ResolverError> {
        let url = Url::parse(rpc_url).map_err(|e| {
            ResolverError::RpcError(format!("Invalid RPC URL: {}", e))
        })?;
        let network = NetworkConfig::from_rpc_url("custom", url);

        Ok(Self { network, cache })
    }

    /// Known NEAR TLDs (whitelist approach for safety)
    /// This prevents accidental resolution of traditional domains through NEAR
    /// even if those TLDs happen to exist as NEAR accounts (like "com" on testnet)
    const KNOWN_NEAR_TLDS: &'static [&'static str] = &[
        // Mainnet
        "near",
        // Testnet
        "testnet",
        // Other NEAR ecosystem TLDs
        "aurora",
        "tg",
        "sweat",
        "kaiching",
        "sharddog",
    ];

    /// Check if a TLD is a known NEAR TLD
    pub fn is_near_tld(&self, tld: &str) -> bool {
        let is_near = Self::KNOWN_NEAR_TLDS.contains(&tld.to_lowercase().as_str());
        debug!(tld = %tld, is_near = %is_near, "TLD check");
        is_near
    }

    /// Check if a contract exists
    async fn contract_exists(&self, contract_id: &str) -> bool {
        // Check cache first
        if let Some(cached) = self.cache.get_contract(contract_id).await {
            debug!(contract_id = %contract_id, cached = %cached, "Contract cache hit");
            return cached;
        }

        let account_id = match near_api::AccountId::from_str(contract_id) {
            Ok(id) => id,
            Err(_) => {
                self.cache.insert_contract(contract_id.to_string(), false).await;
                return false;
            }
        };

        // Check if account exists and has code deployed
        // We check by trying to call a view function - if the contract doesn't have code,
        // we'll get a specific error that we can distinguish
        let exists = Account(account_id).view().fetch_from(&self.network).await.is_ok();

        debug!(contract_id = %contract_id, exists = %exists, "Contract existence check");
        self.cache.insert_contract(contract_id.to_string(), exists).await;
        exists
    }

    /// Query DNS records from a specific contract
    pub async fn query_contract(
        &self,
        contract_id: &str,
        dns_name: &str,
        record_type: &str,
    ) -> Result<Option<Vec<DnsRecord>>, ResolverError> {
        // Check cache first
        if let Some(cached) = self.cache.get_records(contract_id, dns_name, record_type).await {
            debug!(
                contract_id = %contract_id,
                dns_name = %dns_name,
                record_type = %record_type,
                count = cached.len(),
                "Record cache hit"
            );
            return Ok(if cached.is_empty() { None } else { Some(cached) });
        }

        // Check if contract exists first (to avoid unnecessary RPC calls)
        if !self.contract_exists(contract_id).await {
            debug!(contract_id = %contract_id, "Contract does not exist");
            return Ok(None);
        }

        let account_id = near_api::AccountId::from_str(contract_id)
            .map_err(|_| ResolverError::InvalidAccountId(contract_id.to_string()))?;

        let contract = Contract(account_id);

        debug!(
            contract_id = %contract_id,
            dns_name = %dns_name,
            record_type = %record_type,
            "Querying contract"
        );

        let result = contract
            .call_function(
                "dns_query",
                json!({
                    "name": dns_name,
                    "record_type": record_type
                }),
            )
            .read_only()
            .fetch_from(&self.network)
            .await;

        match result {
            Ok(data) => {
                let records: Option<Vec<DnsRecord>> = data.data;
                
                // Cache the result (even if empty)
                let to_cache = records.clone().unwrap_or_default();
                self.cache
                    .insert_records(
                        contract_id.to_string(),
                        dns_name.to_string(),
                        record_type.to_string(),
                        to_cache,
                    )
                    .await;

                if let Some(ref recs) = records {
                    info!(
                        contract_id = %contract_id,
                        dns_name = %dns_name,
                        record_type = %record_type,
                        count = recs.len(),
                        "Found records"
                    );
                }

                Ok(records)
            }
            Err(e) => {
                let err_str = e.to_string();
                warn!(
                    contract_id = %contract_id,
                    error = %err_str,
                    "Contract query failed"
                );

                // If it's a "method not found" or "account doesn't exist" error, return None
                if err_str.contains("MethodNotFound")
                    || err_str.contains("AccountDoesNotExist")
                    || err_str.contains("does not exist")
                    || err_str.contains("CodeDoesNotExist")
                {
                    Ok(None)
                } else {
                    Err(ResolverError::RpcError(err_str))
                }
            }
        }
    }

    /// Generate the resolution order for hierarchical lookup with wildcards
    fn resolution_order<'a>(&self, parts: &'a [&'a str], tld: &'a str) -> Vec<(String, String)> {
        let mut queries = vec![];

        // parts contains all segments except the TLD
        // For "deep.sub.frol.near", parts = ["deep", "sub", "frol"], tld = "near"

        for i in 0..parts.len() {
            // Contract account is parts[i..] joined with dots
            let contract_account = parts[i..].join(".");
            let contract_id = format!("dns.{}.{}", contract_account, tld);

            // DNS name is parts[..i] joined (or "@" if at root)
            let base_name = if i == 0 {
                "@".to_string()
            } else {
                parts[..i].join(".")
            };

            // Try exact match first
            queries.push((contract_id.clone(), base_name.clone()));

            // Then try wildcards (only if we have subdomain parts before this level)
            if i > 0 {
                // Try progressively broader wildcards
                // For i=2 (parts[..2] = ["deep", "sub"]):
                //   - Try "*" (matches anything)
                //   - Try "*.sub" (matches anything under sub)
                // Actually, we want the opposite order: more specific wildcards first
                
                // Most specific wildcard: replace only the leftmost label
                // e.g., "*.sub" for "deep.sub"
                for j in 1..=i {
                    let wildcard = if j == 1 {
                        // Just "*" - matches any single label
                        if i == 1 {
                            "*".to_string()
                        } else {
                            format!("*.{}", parts[1..i].join("."))
                        }
                    } else if j == i {
                        // Full wildcard
                        "*".to_string()
                    } else {
                        // Partial wildcard
                        format!("*.{}", parts[j..i].join("."))
                    };
                    
                    // Avoid duplicates
                    let entry = (contract_id.clone(), wildcard);
                    if !queries.contains(&entry) {
                        queries.push(entry);
                    }
                }
            }
        }

        queries
    }

    /// Resolve a domain name using hierarchical lookup with wildcards
    pub async fn resolve(
        &self,
        domain: &str,
        record_type: &str,
    ) -> Result<Vec<DnsRecord>, ResolverError> {
        // Parse domain: remove trailing dot if present
        let domain = domain.trim_end_matches('.');
        let parts: Vec<&str> = domain.split('.').collect();

        if parts.len() < 2 {
            return Err(ResolverError::InvalidDomain);
        }

        let tld = parts[parts.len() - 1];

        // Check if this is a NEAR TLD
        if !self.is_near_tld(tld) {
            return Err(ResolverError::NotNearTld);
        }

        // Everything except the TLD
        let account_parts = &parts[..parts.len() - 1];

        info!(
            domain = %domain,
            tld = %tld,
            record_type = %record_type,
            "Resolving NEAR domain"
        );

        // Generate resolution order with wildcards
        let resolution_order = self.resolution_order(account_parts, tld);

        debug!(
            resolution_order = ?resolution_order,
            "Resolution order"
        );

        // Try each contract/name combination
        for (contract_id, dns_name) in resolution_order {
            match self.query_contract(&contract_id, &dns_name, record_type).await {
                Ok(Some(records)) if !records.is_empty() => {
                    info!(
                        domain = %domain,
                        contract_id = %contract_id,
                        dns_name = %dns_name,
                        count = records.len(),
                        "Resolved via contract"
                    );
                    return Ok(records);
                }
                Ok(_) => {
                    debug!(
                        contract_id = %contract_id,
                        dns_name = %dns_name,
                        "No records found, trying next"
                    );
                    continue;
                }
                Err(ResolverError::RpcError(e)) => {
                    warn!(
                        contract_id = %contract_id,
                        error = %e,
                        "RPC error, trying next"
                    );
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        info!(domain = %domain, "Domain not found (NXDOMAIN)");
        Err(ResolverError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_resolver() -> NearResolver {
        let cache = DnsCache::new();
        NearResolver::new("https://rpc.testnet.near.org", cache).unwrap()
    }

    #[test]
    fn test_resolution_order_simple() {
        let resolver = create_test_resolver();

        // For "frol.near"
        let order = resolver.resolution_order(&["frol"], "near");
        assert_eq!(order, vec![("dns.frol.near".to_string(), "@".to_string())]);
    }

    #[test]
    fn test_resolution_order_subdomain() {
        let resolver = create_test_resolver();

        // For "www.frol.near"
        let order = resolver.resolution_order(&["www", "frol"], "near");
        
        // Should try:
        // 1. dns.www.frol.near with "@"
        // 2. dns.frol.near with "www"
        // 3. dns.frol.near with "*"
        assert!(order.contains(&("dns.www.frol.near".to_string(), "@".to_string())));
        assert!(order.contains(&("dns.frol.near".to_string(), "www".to_string())));
        assert!(order.contains(&("dns.frol.near".to_string(), "*".to_string())));
    }

    #[test]
    fn test_resolution_order_deep_subdomain() {
        let resolver = create_test_resolver();

        // For "deep.sub.frol.near"
        let order = resolver.resolution_order(&["deep", "sub", "frol"], "near");

        // Should include hierarchical lookups with wildcards
        assert!(order.contains(&("dns.deep.sub.frol.near".to_string(), "@".to_string())));
        assert!(order.contains(&("dns.sub.frol.near".to_string(), "deep".to_string())));
        assert!(order.contains(&("dns.sub.frol.near".to_string(), "*".to_string())));
        assert!(order.contains(&("dns.frol.near".to_string(), "deep.sub".to_string())));
        assert!(order.contains(&("dns.frol.near".to_string(), "*".to_string())));
    }
}
