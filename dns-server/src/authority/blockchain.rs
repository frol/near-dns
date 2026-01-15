use async_trait::async_trait;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::{LowerName, Name, Record, RecordType};
use hickory_server::authority::{
    Authority, LookupControlFlow, LookupObject, LookupOptions, MessageRequest, UpdateResult,
    ZoneType,
};
use hickory_server::server::RequestInfo;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::records::{record_type_to_string, DnsRecord};
use crate::resolver::near::{NearResolver, ResolverError};
use crate::resolver::upstream::{UpstreamError, UpstreamResolver};

/// A lookup result that can be returned from the authority
pub struct BlockchainLookup {
    records: Vec<Record>,
}

impl BlockchainLookup {
    pub fn new(records: Vec<Record>) -> Self {
        Self { records }
    }
}

impl LookupObject for BlockchainLookup {
    fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Record> + Send + 'a> {
        Box::new(self.records.iter())
    }

    fn take_additionals(&mut self) -> Option<Box<dyn LookupObject>> {
        None
    }
}

/// Blockchain DNS Authority
/// 
/// This authority handles all DNS queries by:
/// 1. Checking if the TLD is a NEAR account (dynamic detection)
/// 2. If yes: resolve via NEAR blockchain contracts
/// 3. If no: forward to upstream DNS servers
pub struct BlockchainAuthority {
    origin: LowerName,
    near_resolver: Arc<NearResolver>,
    upstream_resolver: Arc<UpstreamResolver>,
}

impl BlockchainAuthority {
    /// Create a new blockchain authority
    pub fn new(near_resolver: NearResolver, upstream_resolver: UpstreamResolver) -> Self {
        Self {
            origin: LowerName::from(Name::root()),
            near_resolver: Arc::new(near_resolver),
            upstream_resolver: Arc::new(upstream_resolver),
        }
    }

    /// Extract TLD from a domain name
    fn extract_tld(name: &LowerName) -> Option<String> {
        let name_str = name.to_string();
        let name_str = name_str.trim_end_matches('.');
        let parts: Vec<&str> = name_str.split('.').collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts[parts.len() - 1].to_string())
        }
    }

    /// Convert contract DnsRecords to hickory Records
    fn convert_records(
        records: Vec<DnsRecord>,
        name: &Name,
        origin: &Name,
    ) -> Vec<Record> {
        records
            .into_iter()
            .filter_map(|r| match r.to_dns_record(name, origin) {
                Ok(record) => Some(record),
                Err(e) => {
                    warn!(error = %e, "Failed to convert record");
                    None
                }
            })
            .collect()
    }

    /// Handle NEAR domain resolution
    async fn resolve_near(
        &self,
        name: &LowerName,
        rtype: RecordType,
    ) -> LookupControlFlow<BlockchainLookup> {
        let domain = name.to_string();
        let record_type_str = record_type_to_string(rtype);

        debug!(domain = %domain, record_type = %record_type_str, "Resolving NEAR domain");

        match self.near_resolver.resolve(&domain, &record_type_str).await {
            Ok(records) => {
                let origin = Name::from_str(&domain).unwrap_or_else(|_| Name::root());
                let name = Name::from(name.clone());
                let dns_records = Self::convert_records(records, &name, &origin);

                if dns_records.is_empty() {
                    debug!(domain = %domain, "No valid records after conversion");
                    LookupControlFlow::Break(Err(hickory_server::authority::LookupError::from(
                        ResponseCode::NXDomain,
                    )))
                } else {
                    info!(domain = %domain, count = dns_records.len(), "Resolved NEAR domain");
                    LookupControlFlow::Break(Ok(BlockchainLookup::new(dns_records)))
                }
            }
            Err(ResolverError::NotFound) => {
                debug!(domain = %domain, "NEAR domain not found (NXDOMAIN)");
                LookupControlFlow::Break(Err(hickory_server::authority::LookupError::from(
                    ResponseCode::NXDomain,
                )))
            }
            Err(ResolverError::NotNearTld) => {
                // This shouldn't happen if we checked is_near_tld first
                debug!(domain = %domain, "Not a NEAR TLD, forwarding upstream");
                self.resolve_upstream(name, rtype).await
            }
            Err(e) => {
                error!(domain = %domain, error = %e, "NEAR resolution failed");
                LookupControlFlow::Break(Err(hickory_server::authority::LookupError::from(
                    ResponseCode::ServFail,
                )))
            }
        }
    }

    /// Handle upstream DNS resolution
    async fn resolve_upstream(
        &self,
        name: &LowerName,
        rtype: RecordType,
    ) -> LookupControlFlow<BlockchainLookup> {
        let domain = name.to_string();

        debug!(domain = %domain, record_type = ?rtype, "Resolving via upstream DNS");

        match self.upstream_resolver.resolve(&domain, rtype).await {
            Ok(records) => {
                info!(domain = %domain, count = records.len(), "Resolved via upstream");
                LookupControlFlow::Break(Ok(BlockchainLookup::new(records)))
            }
            Err(UpstreamError::NotFound) => {
                debug!(domain = %domain, "Upstream returned no records");
                LookupControlFlow::Break(Err(hickory_server::authority::LookupError::from(
                    ResponseCode::NXDomain,
                )))
            }
            Err(e) => {
                error!(domain = %domain, error = %e, "Upstream resolution failed");
                LookupControlFlow::Break(Err(hickory_server::authority::LookupError::from(
                    ResponseCode::ServFail,
                )))
            }
        }
    }
}

#[async_trait]
impl Authority for BlockchainAuthority {
    type Lookup = BlockchainLookup;

    fn zone_type(&self) -> ZoneType {
        ZoneType::Primary
    }

    fn is_axfr_allowed(&self) -> bool {
        false
    }

    fn origin(&self) -> &LowerName {
        &self.origin
    }

    async fn lookup(
        &self,
        name: &LowerName,
        rtype: RecordType,
        _lookup_options: LookupOptions,
    ) -> LookupControlFlow<Self::Lookup> {
        let domain = name.to_string();
        info!(domain = %domain, record_type = ?rtype, "DNS lookup request");

        // Extract TLD and check if it's a known NEAR TLD
        if let Some(tld) = Self::extract_tld(name) {
            if self.near_resolver.is_near_tld(&tld) {
                debug!(tld = %tld, "TLD is a known NEAR TLD, resolving via blockchain");
                return self.resolve_near(name, rtype).await;
            }
        }

        // Not a NEAR TLD, forward upstream
        debug!(domain = %domain, "Not a NEAR TLD, forwarding to upstream DNS");
        self.resolve_upstream(name, rtype).await
    }

    async fn search(
        &self,
        request: RequestInfo<'_>,
        lookup_options: LookupOptions,
    ) -> LookupControlFlow<Self::Lookup> {
        let name = request.query.name();
        let rtype = request.query.query_type();
        self.lookup(name, rtype, lookup_options).await
    }

    async fn get_nsec_records(
        &self,
        _name: &LowerName,
        _lookup_options: LookupOptions,
    ) -> LookupControlFlow<Self::Lookup> {
        LookupControlFlow::Skip
    }

    async fn update(&self, _update: &MessageRequest) -> UpdateResult<bool> {
        // Updates not supported - use smart contract directly
        Err(ResponseCode::NotImp)
    }
}
