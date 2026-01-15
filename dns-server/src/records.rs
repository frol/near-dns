use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use hickory_proto::rr::rdata::{A, AAAA, CNAME, MX, NS, PTR, SOA, SRV, TXT};
use hickory_proto::rr::{Name, RData, Record, RecordType};

/// DNS record as stored in the NEAR contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub record_type: String,
    pub value: String,
    pub ttl: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
}

/// Error type for record conversion
#[derive(Debug, thiserror::Error)]
pub enum RecordConversionError {
    #[error("Invalid IPv4 address: {0}")]
    InvalidIpv4(String),
    #[error("Invalid IPv6 address: {0}")]
    InvalidIpv6(String),
    #[error("Invalid domain name: {0}")]
    InvalidDomainName(String),
    #[error("Invalid record format for {0}: {1}")]
    InvalidFormat(String, String),
    #[error("Unsupported record type: {0}")]
    UnsupportedType(String),
}

impl DnsRecord {
    /// Convert a contract DnsRecord to hickory RData
    pub fn to_rdata(&self, origin: &Name) -> Result<RData, RecordConversionError> {
        let rtype = self.record_type.to_uppercase();
        match rtype.as_str() {
            "A" => {
                let addr: Ipv4Addr = self
                    .value
                    .parse()
                    .map_err(|_| RecordConversionError::InvalidIpv4(self.value.clone()))?;
                Ok(RData::A(A(addr)))
            }
            "AAAA" => {
                let addr: Ipv6Addr = self
                    .value
                    .parse()
                    .map_err(|_| RecordConversionError::InvalidIpv6(self.value.clone()))?;
                Ok(RData::AAAA(AAAA(addr)))
            }
            "CNAME" => {
                let name = parse_domain_name(&self.value, origin)?;
                Ok(RData::CNAME(CNAME(name)))
            }
            "NS" => {
                let name = parse_domain_name(&self.value, origin)?;
                Ok(RData::NS(NS(name)))
            }
            "PTR" => {
                let name = parse_domain_name(&self.value, origin)?;
                Ok(RData::PTR(PTR(name)))
            }
            "MX" => {
                let exchange = parse_domain_name(&self.value, origin)?;
                let preference = self.priority.unwrap_or(10);
                Ok(RData::MX(MX::new(preference, exchange)))
            }
            "TXT" => {
                Ok(RData::TXT(TXT::new(vec![self.value.clone()])))
            }
            "SRV" => {
                // Format: "weight port target"
                let parts: Vec<&str> = self.value.split_whitespace().collect();
                if parts.len() != 3 {
                    return Err(RecordConversionError::InvalidFormat(
                        "SRV".to_string(),
                        "expected 'weight port target'".to_string(),
                    ));
                }
                let weight: u16 = parts[0]
                    .parse()
                    .map_err(|_| RecordConversionError::InvalidFormat("SRV".to_string(), "invalid weight".to_string()))?;
                let port: u16 = parts[1]
                    .parse()
                    .map_err(|_| RecordConversionError::InvalidFormat("SRV".to_string(), "invalid port".to_string()))?;
                let target = parse_domain_name(parts[2], origin)?;
                let priority = self.priority.unwrap_or(10);
                Ok(RData::SRV(SRV::new(priority, weight, port, target)))
            }
            "SOA" => {
                // Format: "mname rname serial refresh retry expire minimum"
                let parts: Vec<&str> = self.value.split_whitespace().collect();
                if parts.len() != 7 {
                    return Err(RecordConversionError::InvalidFormat(
                        "SOA".to_string(),
                        "expected 'mname rname serial refresh retry expire minimum'".to_string(),
                    ));
                }
                let mname = parse_domain_name(parts[0], origin)?;
                let rname = parse_domain_name(parts[1], origin)?;
                let serial: u32 = parts[2].parse().map_err(|_| {
                    RecordConversionError::InvalidFormat("SOA".to_string(), "invalid serial".to_string())
                })?;
                let refresh: i32 = parts[3].parse().map_err(|_| {
                    RecordConversionError::InvalidFormat("SOA".to_string(), "invalid refresh".to_string())
                })?;
                let retry: i32 = parts[4].parse().map_err(|_| {
                    RecordConversionError::InvalidFormat("SOA".to_string(), "invalid retry".to_string())
                })?;
                let expire: i32 = parts[5].parse().map_err(|_| {
                    RecordConversionError::InvalidFormat("SOA".to_string(), "invalid expire".to_string())
                })?;
                let minimum: u32 = parts[6].parse().map_err(|_| {
                    RecordConversionError::InvalidFormat("SOA".to_string(), "invalid minimum".to_string())
                })?;
                Ok(RData::SOA(SOA::new(mname, rname, serial, refresh, retry, expire, minimum)))
            }
            _ => Err(RecordConversionError::UnsupportedType(self.record_type.clone())),
        }
    }

    /// Convert to a full DNS Record
    pub fn to_dns_record(&self, name: &Name, origin: &Name) -> Result<Record, RecordConversionError> {
        let rdata = self.to_rdata(origin)?;
        Ok(Record::from_rdata(name.clone(), self.ttl, rdata))
    }
}

/// Parse a domain name, handling relative names by appending origin
fn parse_domain_name(name: &str, origin: &Name) -> Result<Name, RecordConversionError> {
    // Handle special case "@" which means the origin itself
    if name == "@" {
        return Ok(origin.clone());
    }
    
    // If the name ends with a dot, it's fully qualified
    if name.ends_with('.') {
        Name::from_str(name)
            .map_err(|_| RecordConversionError::InvalidDomainName(name.to_string()))
    } else {
        // Relative name - append origin
        let full_name = format!("{}.{}", name, origin);
        Name::from_str(&full_name)
            .map_err(|_| RecordConversionError::InvalidDomainName(full_name))
    }
}

/// Convert RecordType enum to string
pub fn record_type_to_string(rt: RecordType) -> String {
    match rt {
        RecordType::A => "A",
        RecordType::AAAA => "AAAA",
        RecordType::CNAME => "CNAME",
        RecordType::MX => "MX",
        RecordType::NS => "NS",
        RecordType::PTR => "PTR",
        RecordType::SOA => "SOA",
        RecordType::SRV => "SRV",
        RecordType::TXT => "TXT",
        RecordType::CAA => "CAA",
        _ => "UNKNOWN",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_record_conversion() {
        let record = DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        };
        let origin = Name::from_str("example.near.").unwrap();
        let rdata = record.to_rdata(&origin).unwrap();
        assert!(matches!(rdata, RData::A(_)));
    }

    #[test]
    fn test_txt_record_conversion() {
        let record = DnsRecord {
            record_type: "TXT".to_string(),
            value: "Hello from NEAR!".to_string(),
            ttl: 300,
            priority: None,
        };
        let origin = Name::from_str("example.near.").unwrap();
        let rdata = record.to_rdata(&origin).unwrap();
        assert!(matches!(rdata, RData::TXT(_)));
    }

    #[test]
    fn test_mx_record_conversion() {
        let record = DnsRecord {
            record_type: "MX".to_string(),
            value: "mail.example.com.".to_string(),
            ttl: 300,
            priority: Some(10),
        };
        let origin = Name::from_str("example.near.").unwrap();
        let rdata = record.to_rdata(&origin).unwrap();
        assert!(matches!(rdata, RData::MX(_)));
    }
}
