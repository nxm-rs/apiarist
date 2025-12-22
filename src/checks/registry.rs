//! Check registry
//!
//! Central registry of all available checks. New checks should be registered here.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

use super::PingpongCheck;
use super::traits::Check;

/// Global registry of all available checks
pub static CHECKS: Lazy<HashMap<&'static str, Arc<dyn Check>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, Arc<dyn Check>> = HashMap::new();

    // Core Protocol checks
    m.insert("pingpong", Arc::new(PingpongCheck));
    // TODO: Add more checks as they are implemented
    // m.insert("peercount", Arc::new(PeercountCheck));
    // m.insert("kademlia", Arc::new(KademliaCheck));
    // m.insert("fullconnectivity", Arc::new(FullconnectivityCheck));

    // Content checks
    // m.insert("retrieval", Arc::new(RetrievalCheck));
    // m.insert("pushsync", Arc::new(PushsyncCheck));
    // m.insert("pullsync", Arc::new(PullsyncCheck));

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
    fn test_pingpong_registered() {
        assert!(CHECKS.contains_key("pingpong"));
    }

    #[test]
    fn test_get_check() {
        let check = get_check("pingpong");
        assert!(check.is_some());
        assert_eq!(check.unwrap().name(), "pingpong");
    }

    #[test]
    fn test_list_checks() {
        let names = list_checks();
        assert!(names.contains(&"pingpong"));
    }
}
