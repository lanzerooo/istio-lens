use super::{AuditIssue, IssueSeverity};
use crate::k8s::ClusterSnapshot;
use itertools::Itertools;
use kube::ResourceExt;
use std::collections::HashMap;

pub fn detect_duplicates(snapshot: &ClusterSnapshot) -> Vec<AuditIssue> {
    let mut issues = Vec::new();

    // 1. Поиск дублирующихся DestinationRule для одного хоста
    let mut dr_by_host: HashMap<String, Vec<&crate::model::DestinationRule>> = HashMap::new();
    for dr in &snapshot.destination_rules {
        let host = &dr.spec.host;
        dr_by_host.entry(host.clone()).or_default().push(dr);
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
                        "Конфликт: Несколько DestinationRule определены для целевого хоста '{}'",
                        host
                    ),
                });
            }
        }
    }

    // 2. Поиск пересекающихся VirtualService (по связке Gateway + Host + Match URI)
    for (vs1, vs2) in snapshot.virtual_services.iter().tuple_combinations() {
        let gws1 = vs1.spec.gateways.clone().unwrap_or_else(|| vec!["mesh".into()]);
        let gws2 = vs2.spec.gateways.clone().unwrap_or_else(|| vec!["mesh".into()]);

        let common_gateways: Vec<_> = gws1.iter().filter(|g| gws2.contains(g)).collect();
        if common_gateways.is_empty() {
            continue;
        }

        let common_hosts: Vec<_> = vs1
            .spec
            .hosts
            .iter()
            .filter(|h| vs2.spec.hosts.contains(h))
            .collect();

        if !common_hosts.is_empty() {
            issues.push(AuditIssue {
                kind: "VirtualService".into(),
                resource: vs1.name_any(),
                namespace: vs1.namespace().unwrap_or_default(),
                severity: IssueSeverity::Warning,
                description: format!(
                    "Потенциальный конфликт маршрутизации с VS '{}/{}' по хостам {:?} на шлюзах {:?}",
                    vs2.namespace().unwrap_or_default(),
                    vs2.name_any(),
                    common_hosts,
                    common_gateways
                ),
            });
        }
    }

    issues
}