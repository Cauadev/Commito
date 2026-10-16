//! Per repository/scope cache of loaded commits. Invalidated when refs/HEAD change.

use super::log::CommitInfo;

#[derive(Default)]
pub struct GraphCache {
    pub fingerprint: u64,
    pub commits: Vec<CommitInfo>,
    pub exhausted: bool,
}

pub fn fingerprint(parts: &[&str]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in parts {
        p.hash(&mut h);
    }
    h.finish()
}
