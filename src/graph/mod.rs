use crate::k8s::{ClusterSnapshot, PodInfo, ServiceMeta};
use crate::model::ResourceKey;
use kube::ResourceExt;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct RouteTrace {
    pub gateway_name: String,
    pub gateway_hosts: Vec<String>,
    pub vs_name: String,
    pub vs_namespace: String,
    pub uri_match: String,
    pub service_host: String,
    pub targets: Vec<SubnetTarget>,
}

#[derive(Clone, Debug)]
pub struct SubnetTarget {
    pub subset_name: String,
    pub weight: i32,
    pub destination_rule: Option<String>,
    pub service_meta: Option<ServiceMeta>,
    pub matching_pods: Vec<PodInfo>,
}

pub struct TrafficGraph;

impl TrafficGraph {
    pub fn build_traces(snapshot: &ClusterSnapshot, filter_ns: Option<&str>) -> Vec<RouteTrace> {
        let mut traces = Vec::new();

        for vs in &snapshot.virtual_services {
            let vs_ns = vs.namespace().unwrap_or_default();
            if filter_ns.is_some() && vs.namespace().as_deref() != filter_ns {
                continue;
            }

            let gws = vs.spec.gateways.clone().unwrap_or_else(|| vec!["mesh".into()]);

            if let Some(http_routes) = &vs.spec.http {
                for route in http_routes {
                    let uri_match = route
                        .r#match
                        .as_ref()
                        .and_then(|m| m.first())
                        .and_then(|m| m.uri.as_ref())
                        .map(|u| match u {
                            crate::model::StringMatch::Exact(s) => format!("Exact({})", s),
                            crate::model::StringMatch::Prefix(s) => format!("Prefix({})", s),
                            crate::model::StringMatch::Regex(s) => format!("Regex({})", s),
                        })
                        .unwrap_or_else(|| "/* (All)".into());

                    if let Some(destinations) = &route.route {
                        for dest in destinations {
                            let host = &dest.destination.host;
                            let subset_req = dest.destination.subset.clone();
                            let weight = dest.weight.unwrap_or(100);

                            // Поиск метаданных Service
                            let svc_name = host.split('.').next().unwrap_or(host);
                            let svc_key = ResourceKey::new(&vs_ns, svc_name);
                            let svc_meta = snapshot.services_meta.get(&svc_key).cloned();

                            // Поиск DestinationRule для этого хоста
                            let dr = snapshot.destination_rules.iter().find(|d| {
                                d.spec.host == *host || d.spec.host.starts_with(svc_name)
                            });

                            // Определение селекторов пода
                            let mut target_selector = BTreeMap::new();
                            if let Some(ref meta) = svc_meta {
                                target_selector.extend(meta.selector.clone());
                            }

                            if let Some(dr_rule) = dr {
                                if let Some(ref sub_name) = subset_req {
                                    if let Some(subsets) = &dr_rule.spec.subsets {
                                        if let Some(found_sub) = subsets.iter().find(|s| s.name == *sub_name) {
                                            if let Some(labels) = &found_sub.labels {
                                                target_selector.extend(labels.clone());
                                            }
                                        }
                                    }
                                }
                            }

                            // Поиск реальных подов, подходящих под селекторы
                            let mut matching_pods = Vec::new();
                            if !target_selector.is_empty() {
                                for pod in &snapshot.pods {
                                    if pod.namespace == vs_ns {
                                        let all_match = target_selector.iter().all(|(k, v)| {
                                            pod.labels.get(k) == Some(v)
                                        });
                                        if all_match {
                                            matching_pods.push(pod.clone());
                                        }
                                    }
                                }
                            }

                            for gw in &gws {
                                traces.push(RouteTrace {
                                    gateway_name: gw.clone(),
                                    gateway_hosts: vs.spec.hosts.clone(),
                                    vs_name: vs.name_any(),
                                    vs_namespace: vs_ns.clone(),
                                    uri_match: uri_match.clone(),
                                    service_host: host.clone(),
                                    targets: vec![SubnetTarget {
                                        subset_name: subset_req.clone().unwrap_or_else(|| "default".into()),
                                        weight,
                                        destination_rule: dr.map(|d| d.name_any()),
                                        service_meta: svc_meta.clone(),
                                        matching_pods: matching_pods.clone(), 
                                    }],
                                });
                            }
                        }
                    }
                }
            }
        }

        traces
    }
}