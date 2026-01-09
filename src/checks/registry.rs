//! Check registry
//!
//! Central registry of all available checks. New checks should be registered here.
//!
//! ## Check Ordering
//!
//! Checks are registered in a specific order to optimize test execution:
//! 1. **Connectivity checks** run first (pingpong, peercount, fullconnectivity)
//!    These verify the network is up before content operations.
//! 2. **Content checks** run after connectivity is verified (smoke, pushsync, retrieval)
//!    These require a stable network for reliable results.
//!
//! This ordering prevents slow pushsync delays when uploading chunks to an
//! unstable network.

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use std::sync::Arc;

use super::traits::Check;
use super::{
    FullconnectivityCheck, PeercountCheck, PingpongCheck, PullsyncCheck, PushsyncCheck,
    RetrievalCheck, SmokeCheck,
};

/// Global registry of all available checks
///
/// Uses IndexMap to preserve insertion order, ensuring checks run in a
/// deterministic sequence (connectivity before content).
pub static CHECKS: Lazy<IndexMap<&'static str, Arc<dyn Check>>> = Lazy::new(|| {
    let mut m: IndexMap<&'static str, Arc<dyn Check>> = IndexMap::new();

    // P0: Core Protocol checks - run FIRST to verify network connectivity
    m.insert("pingpong", Arc::new(PingpongCheck));
    m.insert("peercount", Arc::new(PeercountCheck));
    m.insert("fullconnectivity", Arc::new(FullconnectivityCheck));

    // P1: Content checks - run AFTER connectivity is verified
    // This prevents slow pushsync delays on unstable networks
    m.insert("smoke", Arc::new(SmokeCheck));
    m.insert("pushsync", Arc::new(PushsyncCheck));
    m.insert("pullsync", Arc::new(PullsyncCheck));
    m.insert("retrieval", Arc::new(RetrievalCheck));
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
        assert!(CHECKS.contains_key("fullconnectivity"));
    }

    #[test]
    fn test_p1_checks_registered() {
        assert!(CHECKS.contains_key("smoke"));
        assert!(CHECKS.contains_key("pushsync"));
        assert!(CHECKS.contains_key("pullsync"));
        assert!(CHECKS.contains_key("retrieval"));
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
        assert!(names.contains(&"fullconnectivity"));
    }
}
