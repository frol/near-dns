use hickory_resolver::config::ResolverConfig;
use hickory_resolver::TokioResolver;
use std::str::FromStr;
use tracing::{debug, info};

use hickory_proto::rr::{Name, RData, Record, RecordType};

/// Error type for upstream resolution
#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("Resolution failed: {0}")]
    ResolutionFailed(String),
    #[error("No records found")]
    NotFound,
}

/// Upstream DNS resolver for non-NEAR domains
pub struct UpstreamResolver {
    resolver: TokioResolver,
}

impl UpstreamResolver {
    /// Create a new upstream resolver with default DNS servers (Google, Cloudflare)
    pub fn new() -> Self {
        let resolver = TokioResolver::builder_with_config(
            ResolverConfig::default(),
            hickory_resolver::name_server::TokioConnectionProvider::default(),
        ).build();
        Self { resolver }
    }

    /// Resolve a domain using upstream DNS
    pub async fn resolve(
        &self,
        domain: &str,
        record_type: RecordType,
    ) -> Result<Vec<Record>, UpstreamError> {
        let name = Name::from_str(domain)
            .map_err(|e| UpstreamError::ResolutionFailed(format!("Invalid domain: {}", e)))?;

        debug!(domain = %domain, record_type = ?record_type, "Resolving via upstream DNS");

        match record_type {
            RecordType::A => {
                let response = self
                    .resolver
                    .ipv4_lookup(domain)
                    .await
                    .map_err(|e| UpstreamError::ResolutionFailed(e.to_string()))?;

                let records: Vec<Record> = response
                    .iter()
                    .map(|ip| {
                        Record::from_rdata(
                            name.clone(),
                            300,
                            RData::A(hickory_proto::rr::rdata::A(ip.0)),
                        )
                    })
                    .collect();

                if records.is_empty() {
                    Err(UpstreamError::NotFound)
                } else {
                    info!(domain = %domain, count = records.len(), "Resolved A records from upstream");
                    Ok(records)
                }
            }
            RecordType::AAAA => {
                let response = self
                    .resolver
                    .ipv6_lookup(domain)
                    .await
                    .map_err(|e| UpstreamError::ResolutionFailed(e.to_string()))?;

                let records: Vec<Record> = response
                    .iter()
                    .map(|ip| {
                        Record::from_rdata(
                            name.clone(),
                            300,
                            RData::AAAA(hickory_proto::rr::rdata::AAAA(ip.0)),
                        )
                    })
                    .collect();

                if records.is_empty() {
                    Err(UpstreamError::NotFound)
                } else {
                    info!(domain = %domain, count = records.len(), "Resolved AAAA records from upstream");
                    Ok(records)
                }
            }
            RecordType::MX => {
                let response = self
                    .resolver
                    .mx_lookup(domain)
                    .await
                    .map_err(|e| UpstreamError::ResolutionFailed(e.to_string()))?;

                let records: Vec<Record> = response
                    .iter()
                    .map(|mx| {
                        Record::from_rdata(
                            name.clone(),
                            300,
                            RData::MX(hickory_proto::rr::rdata::MX::new(
                                mx.preference(),
                                mx.exchange().clone(),
                            )),
                        )
                    })
                    .collect();

                if records.is_empty() {
                    Err(UpstreamError::NotFound)
                } else {
                    info!(domain = %domain, count = records.len(), "Resolved MX records from upstream");
                    Ok(records)
                }
            }
            RecordType::TXT => {
                let response = self
                    .resolver
                    .txt_lookup(domain)
                    .await
                    .map_err(|e| UpstreamError::ResolutionFailed(e.to_string()))?;

                let records: Vec<Record> = response
                    .iter()
                    .map(|txt| {
                        // Convert TXT record data (bytes) to strings
                        let txt_data: Vec<String> = txt
                            .iter()
                            .filter_map(|bytes| String::from_utf8(bytes.to_vec()).ok())
                            .collect();
                        Record::from_rdata(
                            name.clone(),
                            300,
                            RData::TXT(hickory_proto::rr::rdata::TXT::new(txt_data)),
                        )
                    })
                    .collect();

                if records.is_empty() {
                    Err(UpstreamError::NotFound)
                } else {
                    info!(domain = %domain, count = records.len(), "Resolved TXT records from upstream");
                    Ok(records)
                }
            }
            RecordType::NS => {
                let response = self
                    .resolver
                    .ns_lookup(domain)
                    .await
                    .map_err(|e| UpstreamError::ResolutionFailed(e.to_string()))?;

                let records: Vec<Record> = response
                    .iter()
                    .map(|ns| {
                        Record::from_rdata(
                            name.clone(),
                            300,
                            RData::NS(hickory_proto::rr::rdata::NS(ns.0.clone())),
                        )
                    })
                    .collect();

                if records.is_empty() {
                    Err(UpstreamError::NotFound)
                } else {
                    info!(domain = %domain, count = records.len(), "Resolved NS records from upstream");
                    Ok(records)
                }
            }
            RecordType::SOA => {
                let response = self
                    .resolver
                    .soa_lookup(domain)
                    .await
                    .map_err(|e| UpstreamError::ResolutionFailed(e.to_string()))?;

                let records: Vec<Record> = response
                    .iter()
                    .map(|soa| {
                        Record::from_rdata(name.clone(), 300, RData::SOA(soa.clone()))
                    })
                    .collect();

                if records.is_empty() {
                    Err(UpstreamError::NotFound)
                } else {
                    info!(domain = %domain, count = records.len(), "Resolved SOA records from upstream");
                    Ok(records)
                }
            }
            _ => {
                // For other record types, use generic lookup
                let response = self
                    .resolver
                    .lookup(domain, record_type)
                    .await
                    .map_err(|e| UpstreamError::ResolutionFailed(e.to_string()))?;

                let records: Vec<Record> = response
                    .record_iter()
                    .filter(|r| r.record_type() == record_type)
                    .cloned()
                    .collect();

                if records.is_empty() {
                    Err(UpstreamError::NotFound)
                } else {
                    info!(domain = %domain, record_type = ?record_type, count = records.len(), "Resolved records from upstream");
                    Ok(records)
                }
            }
        }
    }
}

impl Default for UpstreamResolver {
    fn default() -> Self {
        Self::new()
    }
}
