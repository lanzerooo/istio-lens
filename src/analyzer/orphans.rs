use super::{AuditIssue, IssueSeverity};
use crate::k8s::ClusterSnapshot;
use crate::model::ResourceKey;
use kube::ResourceExt;
use std::collections::HashSet;

pub fn detect_orphans(snapshot: &ClusterSnapshot) -> Vec<AuditIssue> {
    let mut issues = Vec::new();

    let mut referenced_gateways = HashSet::new();
    for vs in &snapshot.virtual_services {
        if let Some(gws) = &vs.spec.gateways {
            for gw in gws {
                referenced_gateways.insert(gw.clone());
            }
        }
    }

    // 1. Поиск неиспользуемых Gateway (на которые не ссылается ни один VS)
    for gw in &snapshot.gateways {
        let name = gw.name_any();
        let ns = gw.namespace().unwrap_or_default();
        let short_ref = name.clone();
        let full_ref = format!("{}/{}", ns, name);

        if !referenced_gateways.contains(&short_ref) && !referenced_gateways.contains(&full_ref) {
            issues.push(AuditIssue {
                kind: "Gateway".into(),
                resource: name,
                namespace: ns,
                severity: IssueSeverity::Warning,
                description: "Шлюз не используется: ни один VirtualService не привязан к нему".into(),
            });
        }
    }

    // 2. VirtualService указывает на несуществующий K8s Service
    for vs in &snapshot.virtual_services {
        let vs_ns = vs.namespace().unwrap_or_else(|| "default".into());
        if let Some(http_routes) = &vs.spec.http {
            for route in http_routes {
                if let Some(destinations) = &route.route {
                    for dest in destinations {
                        let host = &dest.destination.host;
                        if !is_host_resolvable(host, &vs_ns, &snapshot.existing_service_keys) {
                            issues.push(AuditIssue {
                                kind: "VirtualService".into(),
                                resource: vs.name_any(),
                                namespace: vs_ns.clone(),
                                severity: IssueSeverity::Critical,
                                description: format!(
                                    "Битый маршрут: целевой хост '{}' не существует среди K8s Services",
                                    host
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    issues
}

fn is_host_resolvable(host: &str, current_ns: &str, services: &HashSet<ResourceKey>) -> bool {
    if host.contains("external") || host == "mesh" {
        return true;
    }

    let parts: Vec<&str> = host.split('.').collect();
    match parts.len() {
        1 => services.contains(&ResourceKey::new(current_ns, parts[0])),
        2 => services.contains(&ResourceKey::new(parts[1], parts[0])),
        _ => {
            // Разбор FQDN вида name.namespace.svc.cluster.local
            let name = parts[0];
            let namespace = parts[1];
            services.contains(&ResourceKey::new(namespace, name))
        }
    }
}