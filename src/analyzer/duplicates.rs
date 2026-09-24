use super::{AuditIssue, IssueSeverity};
use crate::k8s::ClusterSnapshot;
use kube::ResourceExt;
use std::collections::HashMap;

pub fn detect_duplicates(snapshot: &ClusterSnapshot) -> Vec<AuditIssue> {
    let mut issues = Vec::new();

    // 1. Поиск дубликатов DestinationRule по ключу хоста - O(N)
    let mut dr_by_host: HashMap<&str, Vec<&crate::model::DestinationRule>> = HashMap::new();
    for dr in &snapshot.destination_rules {
        dr_by_host.entry(&dr.spec.host).or_default().push(dr);
    }

    for (host, rules) in dr_by_host {
        if rules.len() > 1 {
            for dr in rules {
                issues.push(AuditIssue {
                    kind: "DestinationRule".into(),
                    resource: dr.name_any(),
                    namespace: dr.namespace().unwrap_or_default(),
                    severity: IssueSeverity::Critical,
                    description: format!(
                        "Конфликт DR: несколько манифестов управляют трафиком хоста '{}'",
                        host
                    ),
                });
            }
        }
    }

    // 2. Инвертированный индекс VirtualService по паре (Gateway, Host) - O(N)
    let mut route_index: HashMap<(String, String), Vec<&crate::model::VirtualService>> = HashMap::new();
    let default_gateways = vec!["mesh".to_string()];

    for vs in &snapshot.virtual_services {
        let gws = vs.spec.gateways.as_deref().unwrap_or(&default_gateways);
        for gw in gws {
            for host in &vs.spec.hosts {
                route_index
                    .entry((gw.clone(), host.clone()))
                    .or_default()
                    .push(vs);
            }
        }
    }

    for ((gw, host), matching_vs) in route_index {
        if matching_vs.len() > 1 {
            for vs in &matching_vs {
                issues.push(AuditIssue {
                    kind: "VirtualService".into(),
                    resource: vs.name_any(),
                    namespace: vs.namespace().unwrap_or_default(),
                    severity: IssueSeverity::Warning,
                    description: format!(
                        "Коллизия маршрута: пересечение для шлюза '{}' и хоста '{}' между несколькими VS",
                        gw, host
                    ),
                });
            }
        }
    }

    issues
}