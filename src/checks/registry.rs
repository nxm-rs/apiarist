//! Check registry
//!
//! Central registry of all available checks. New checks should be registered here.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

use super::traits::Check;
use super::{FullconnectivityCheck, KademliaCheck, PeercountCheck, PingpongCheck};

/// Global registry of all available checks
pub static CHECKS: Lazy<HashMap<&'static str, Arc<dyn Check>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, Arc<dyn Check>> = HashMap::new();

    // P0: Core Protocol checks
    m.insert("pingpong", Arc::new(PingpongCheck));
    m.insert("peercount", Arc::new(PeercountCheck));
    m.insert("kademlia", Arc::new(KademliaCheck));
    m.insert("fullconnectivity", Arc::new(FullconnectivityCheck));

    // P1: Content checks (TODO)
    // m.insert("smoke", Arc::new(SmokeCheck));
    // m.insert("pushsync", Arc::new(PushsyncCheck));
    // m.insert("retrieval", Arc::new(RetrievalCheck));
    // m.insert("fileretrieval", Arc::new(FileretrievalCheck));

    // P2: Advanced features (TODO)
    // m.insert("postage", Arc::new(PostageCheck));
    // m.insert("manifest", Arc::new(ManifestCheck));
    // m.insert("feed", Arc::new(FeedCheck));
    // m.insert("soc", Arc::new(SocCheck));
    // m.insert("pss", Arc::new(PssCheck));

    // P3: Economic operations (TODO)
    // m.insert("balances", Arc::new(BalancesCheck));
    // m.insert("settlements", Arc::new(SettlementsCheck));
    // m.insert("stake", Arc::new(StakeCheck));

    m
});

/// Get a check by name
pub fn get_check(name: &str) -> Option<Arc<dyn Check>> {
    CHECKS.get(name).cloned()
}

/// List all available check names
pub fn list_checks() -> Vec<&'static str> {
    let mut names: Vec<_> = CHECKS.keys().copied().collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p0_checks_registered() {
        assert!(CHECKS.contains_key("pingpong"));
        assert!(CHECKS.contains_key("peercount"));
        assert!(CHECKS.contains_key("kademlia"));
        assert!(CHECKS.contains_key("fullconnectivity"));
    }

    #[test]
    fn test_get_check() {
        let check = get_check("pingpong");
        assert!(check.is_some());
        assert_eq!(check.unwrap().name(), "pingpong");

        let check = get_check("peercount");
        assert!(check.is_some());
        assert_eq!(check.unwrap().name(), "peercount");
    }

    #[test]
    fn test_list_checks() {
        let names = list_checks();
        assert!(names.contains(&"pingpong"));
        assert!(names.contains(&"peercount"));
        assert!(names.contains(&"kademlia"));
        assert!(names.contains(&"fullconnectivity"));
    }
}
