use near_sdk::store::IterableMap;
use near_sdk::{env, near, AccountId, PanicOnDefault};

/// DNS record as stored in the contract
#[near(serializers = [borsh, json])]
#[derive(Clone, Debug)]
pub struct DnsRecord {
    pub record_type: String,
    pub value: String,
    pub ttl: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
}

/// DNS Contract for storing and querying DNS records
/// 
/// This contract is designed to be deployed as `dns.<account>.<tld>` 
/// For example: `dns.alice.near` or `dns.myapp.testnet`
/// 
/// Only the parent account (e.g., `alice.near` for `dns.alice.near`) can update records.
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct DnsContract {
    /// DNS records stored as name:type -> Vec<DnsRecord>
    records: IterableMap<String, Vec<DnsRecord>>,
    /// Owner of this DNS contract (parent account)
    owner: AccountId,
}

#[near]
impl DnsContract {
    /// Initialize the DNS contract
    /// The owner is automatically set to the parent account
    #[init]
    pub fn new() -> Self {
        let current_account = env::current_account_id();
        let owner = current_account.get_parent_account_id()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| env::predecessor_account_id());
        
        env::log_str(&format!("DNS contract initialized. Owner: {}", owner));
        
        Self {
            records: IterableMap::new(b"r"),
            owner,
        }
    }

    /// Check if the caller is the owner
    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner,
            "Only the owner ({}) can modify DNS records",
            self.owner
        );
    }

    /// Build the storage key for a name and record type
    fn make_key(name: &str, record_type: &str) -> String {
        format!("{}:{}", name.to_lowercase(), record_type.to_uppercase())
    }

    // ========== VIEW METHODS ==========

    /// Query DNS records for a given name and record type
    /// 
    /// # Arguments
    /// * `name` - The DNS name to query (e.g., "@" for root, "www", "mail", "*" for wildcard)
    /// * `record_type` - The record type (e.g., "A", "AAAA", "CNAME", "MX", "TXT")
    /// 
    /// # Returns
    /// * `Option<Vec<DnsRecord>>` - The matching records, or None if not found
    pub fn dns_query(&self, name: String, record_type: String) -> Option<Vec<DnsRecord>> {
        let key = Self::make_key(&name, &record_type);
        self.records.get(&key).cloned()
    }

    /// Get all records for a given name (all types)
    pub fn dns_query_all(&self, name: String) -> Vec<DnsRecord> {
        let mut all_records = Vec::new();
        let record_types = ["A", "AAAA", "CNAME", "MX", "NS", "TXT", "SRV", "SOA", "PTR", "CAA"];
        
        for rt in record_types {
            let key = Self::make_key(&name, rt);
            if let Some(records) = self.records.get(&key) {
                all_records.extend(records.clone());
            }
        }
        
        all_records
    }

    /// List all DNS names that have records
    pub fn dns_list_names(&self) -> Vec<String> {
        self.records
            .keys()
            .map(|k| {
                // Extract name from "name:TYPE" key format
                k.split(':').next().unwrap_or(k).to_string()
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// List all records in the contract
    pub fn dns_list_all(&self) -> Vec<(String, Vec<DnsRecord>)> {
        self.records
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the owner of this DNS contract
    pub fn get_owner(&self) -> AccountId {
        self.owner.clone()
    }

    // ========== CHANGE METHODS ==========

    /// Update/set DNS records for a given name and record type
    /// This will replace any existing records of the same type
    /// 
    /// # Arguments
    /// * `name` - The DNS name (e.g., "@", "www", "mail", "*")
    /// * `records` - The records to set
    /// 
    /// # Panics
    /// * If caller is not the owner
    pub fn dns_update(&mut self, name: String, records: Vec<DnsRecord>) {
        self.assert_owner();
        
        // Validate records
        assert!(!records.is_empty(), "Records cannot be empty");
        
        // All records must be of the same type
        let record_type = records[0].record_type.to_uppercase();
        for r in &records {
            assert_eq!(
                r.record_type.to_uppercase(),
                record_type,
                "All records must be of the same type"
            );
        }
        
        let key = Self::make_key(&name, &record_type);
        self.records.insert(key, records.clone());
        
        env::log_str(&format!(
            "Updated {} {} record(s) for name '{}'",
            records.len(),
            record_type,
            name
        ));
    }

    /// Add a single DNS record (appends to existing records of the same type)
    /// 
    /// # Arguments
    /// * `name` - The DNS name
    /// * `record` - The record to add
    pub fn dns_add(&mut self, name: String, record: DnsRecord) {
        self.assert_owner();
        
        let key = Self::make_key(&name, &record.record_type);
        let mut existing = self.records.get(&key).cloned().unwrap_or_default();
        existing.push(record.clone());
        self.records.insert(key, existing);
        
        env::log_str(&format!(
            "Added {} record for name '{}'",
            record.record_type.to_uppercase(),
            name
        ));
    }

    /// Delete DNS records
    /// 
    /// # Arguments
    /// * `name` - The DNS name
    /// * `record_type` - Optional record type. If None, deletes all records for the name
    pub fn dns_delete(&mut self, name: String, record_type: Option<String>) {
        self.assert_owner();
        
        if let Some(rt) = record_type {
            // Delete specific record type
            let key = Self::make_key(&name, &rt);
            self.records.remove(&key);
            env::log_str(&format!("Deleted {} records for name '{}'", rt.to_uppercase(), name));
        } else {
            // Delete all record types for this name
            let record_types = ["A", "AAAA", "CNAME", "MX", "NS", "TXT", "SRV", "SOA", "PTR", "CAA"];
            for rt in record_types {
                let key = Self::make_key(&name, rt);
                self.records.remove(&key);
            }
            env::log_str(&format!("Deleted all records for name '{}'", name));
        }
    }

    /// Transfer ownership to a new account
    /// 
    /// # Arguments
    /// * `new_owner` - The new owner account ID
    pub fn transfer_ownership(&mut self, new_owner: AccountId) {
        self.assert_owner();
        let old_owner = self.owner.clone();
        self.owner = new_owner.clone();
        env::log_str(&format!("Ownership transferred from {} to {}", old_owner, new_owner));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn get_context(predecessor: &str) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(predecessor.parse().unwrap());
        builder.current_account_id("dns.alice.testnet".parse().unwrap());
        builder
    }

    #[test]
    fn test_initialization() {
        let context = get_context("alice.testnet");
        testing_env!(context.build());
        
        let contract = DnsContract::new();
        assert_eq!(contract.get_owner().as_str(), "alice.testnet");
    }

    #[test]
    fn test_dns_update_and_query() {
        let context = get_context("alice.testnet");
        testing_env!(context.build());
        
        let mut contract = DnsContract::new();
        
        // Update A record
        let records = vec![DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        }];
        contract.dns_update("@".to_string(), records.clone());
        
        // Query should return the record
        let result = contract.dns_query("@".to_string(), "A".to_string());
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_dns_add() {
        let context = get_context("alice.testnet");
        testing_env!(context.build());
        
        let mut contract = DnsContract::new();
        
        // Add first A record
        contract.dns_add("@".to_string(), DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        });
        
        // Add second A record
        contract.dns_add("@".to_string(), DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.2".to_string(),
            ttl: 300,
            priority: None,
        });
        
        // Should have 2 records
        let result = contract.dns_query("@".to_string(), "A".to_string());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_dns_delete() {
        let context = get_context("alice.testnet");
        testing_env!(context.build());
        
        let mut contract = DnsContract::new();
        
        // Add records
        contract.dns_add("@".to_string(), DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        });
        contract.dns_add("@".to_string(), DnsRecord {
            record_type: "TXT".to_string(),
            value: "Hello".to_string(),
            ttl: 300,
            priority: None,
        });
        
        // Delete only A records
        contract.dns_delete("@".to_string(), Some("A".to_string()));
        
        // A should be gone, TXT should remain
        assert!(contract.dns_query("@".to_string(), "A".to_string()).is_none());
        assert!(contract.dns_query("@".to_string(), "TXT".to_string()).is_some());
    }

    #[test]
    #[should_panic(expected = "Only the owner")]
    fn test_unauthorized_update() {
        let context = get_context("alice.testnet");
        testing_env!(context.build());
        
        let mut contract = DnsContract::new();
        
        // Change predecessor to someone else
        let context = get_context("bob.testnet");
        testing_env!(context.build());
        
        // This should panic
        contract.dns_update("@".to_string(), vec![DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        }]);
    }

    #[test]
    fn test_wildcard_records() {
        let context = get_context("alice.testnet");
        testing_env!(context.build());
        
        let mut contract = DnsContract::new();
        
        // Add wildcard A record
        contract.dns_add("*".to_string(), DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        });
        
        // Query wildcard
        let result = contract.dns_query("*".to_string(), "A".to_string());
        assert!(result.is_some());
    }

    #[test]
    fn test_dns_list_names() {
        let context = get_context("alice.testnet");
        testing_env!(context.build());
        
        let mut contract = DnsContract::new();
        
        // Add records for different names
        contract.dns_add("@".to_string(), DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.1".to_string(),
            ttl: 300,
            priority: None,
        });
        contract.dns_add("www".to_string(), DnsRecord {
            record_type: "A".to_string(),
            value: "192.168.1.2".to_string(),
            ttl: 300,
            priority: None,
        });
        contract.dns_add("@".to_string(), DnsRecord {
            record_type: "TXT".to_string(),
            value: "test".to_string(),
            ttl: 300,
            priority: None,
        });
        
        let names = contract.dns_list_names();
        assert_eq!(names.len(), 2); // "@" and "www"
        assert!(names.contains(&"@".to_string()));
        assert!(names.contains(&"www".to_string()));
    }
}
