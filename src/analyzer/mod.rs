pub mod duplicates;
pub mod orphans;

use crate::k8s::ClusterSnapshot;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum IssueSeverity {
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditIssue {
    pub kind: String,
    pub resource: String,
    pub namespace: String,
    pub severity: IssueSeverity,
    pub description: String,
}

pub struct Analyzer<'a> {
    snapshot: &'a ClusterSnapshot,
}

impl<'a> Analyzer<'a> {
    pub fn new(snapshot: &'a ClusterSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn run_all(&self) -> (Vec<AuditIssue>, Vec<AuditIssue>) {
        let duplicates = duplicates::detect_duplicates(self.snapshot);
        let orphans = orphans::detect_orphans(self.snapshot);
        (duplicates, orphans)
    }
}