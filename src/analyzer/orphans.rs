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

    // 1. Поиск Gateway без VirtualService
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
                description: "Неиспользуемый шлюз: ни один VirtualService не привязан к нему".into(),
            });
        }
    }

    // 2. Валидация маршрутов VS: существование сервиса + Ready Endpoints
    for vs in &snapshot.virtual_services {
        let vs_ns = vs.namespace().unwrap_or_else(|| "default".into());
        if let Some(http_routes) = &vs.spec.http {
            for route in http_routes {
                if let Some(destinations) = &route.route {
                    for dest in destinations {
                        let host = &dest.destination.host;

                        // Если хост определен в ServiceEntry — маршрут валиден
                        if snapshot.external_hosts.contains(host) {
                            continue;
                        }

                        let resolved_key = resolve_service_key(host, &vs_ns);
                        match resolved_key {
                            Some(key) => {
                                if !snapshot.existing_service_keys.contains(&key) {
                                    issues.push(AuditIssue {
                                        kind: "VirtualService".into(),
                                        resource: vs.name_any(),
                                        namespace: vs_ns.clone(),
                                        severity: IssueSeverity::Critical,
                                        description: format!(
                                            "Битый маршрут: целевой Service '{}/{}' не существует",
                                            key.namespace, key.name
                                        ),
                                    });
                                } else {
                                    // Проверка эндпоинтов (готовых подов)
                                    let ready_pods = snapshot.service_ready_endpoints.get(&key).copied().unwrap_or(0);
                                    if ready_pods == 0 {
                                        issues.push(AuditIssue {
                                            kind: "VirtualService".into(),
                                            resource: vs.name_any(),
                                            namespace: vs_ns.clone(),
                                            severity: IssueSeverity::Critical,
                                            description: format!(
                                                "Маршрут в тупик (Dead End): у сервиса '{}/{}' ровно 0 Ready подов!",
                                                key.namespace, key.name
                                            ),
                                        });
                                    }
                                }
                            }
                            None => {
                                if !host.contains("cluster.local") && !host.contains('.') {
                                    issues.push(AuditIssue {
                                        kind: "VirtualService".into(),
                                        resource: vs.name_any(),
                                        namespace: vs_ns.clone(),
                                        severity: IssueSeverity::Warning,
                                        description: format!("Неразрешимый хост назначения '{}'", host),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    issues
}

fn resolve_service_key(host: &str, current_ns: &str) -> Option<ResourceKey> {
    if host == "mesh" {
        return None;
    }
    let parts: Vec<&str> = host.split('.').collect();
    match parts.len() {
        1 => Some(ResourceKey::new(current_ns, parts[0])),
        2 => Some(ResourceKey::new(parts[1], parts[0])),
        _ => Some(ResourceKey::new(parts[1], parts[0])),
    }
}