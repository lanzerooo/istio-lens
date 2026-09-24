use crate::k8s::ClusterSnapshot;
use crate::model::ResourceKey;
use kube::ResourceExt;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TopologyNode {
    Gateway(String),
    VirtualService(String),
    Service { name: String, ready_endpoints: usize },
    ServiceEntry(String),
    DestinationRule { name: String, subsets: Vec<String> },
}

impl TopologyNode {
    pub fn label(&self) -> String {
        match self {
            Self::Gateway(name) => format!("🌐 Gateway [{}]", name),
            Self::VirtualService(name) => format!("🔀 VirtualService [{}]", name),
            Self::Service { name, ready_endpoints } => {
                let status = if *ready_endpoints > 0 {
                    format!("● {} ready", ready_endpoints)
                } else {
                    "✖ 0 endpoints".to_string()
                };
                format!("⚙️  Service [{}] ({})", name, status)
            }
            Self::ServiceEntry(name) => format!("🌍 ServiceEntry [{}]", name),
            Self::DestinationRule { name, subsets } => {
                let sub_str = if subsets.is_empty() {
                    "default".into()
                } else {
                    subsets.join(", ")
                };
                format!("🎯 DestinationRule [{}] (subsets: {})", name, sub_str)
            }
        }
    }
}

pub struct TrafficGraph {
    pub graph: DiGraph<TopologyNode, &'static str>,
}

impl TrafficGraph {
    pub fn build(snapshot: &ClusterSnapshot, filter_ns: Option<&str>) -> Self {
        let mut graph = DiGraph::new();
        let mut node_indices: HashMap<String, NodeIndex> = HashMap::new();

        // 1. Gateway Nodes
        for gw in &snapshot.gateways {
            if filter_ns.is_some() && gw.namespace().as_deref() != filter_ns {
                continue;
            }
            let key = format!("gw:{}/{}", gw.namespace().unwrap_or_default(), gw.name_any());
            let idx = graph.add_node(TopologyNode::Gateway(gw.name_any()));
            node_indices.insert(key, idx);
        }

        // 2. VirtualServices
        for vs in &snapshot.virtual_services {
            let vs_ns = vs.namespace().unwrap_or_default();
            if filter_ns.is_some() && vs.namespace().as_deref() != filter_ns {
                continue;
            }
            let vs_key = format!("vs:{}/{}", vs_ns, vs.name_any());
            let vs_idx = graph.add_node(TopologyNode::VirtualService(vs.name_any()));
            node_indices.insert(vs_key, vs_idx);

            // Связываем со шлюзами
            if let Some(gws) = &vs.spec.gateways {
                for gw_name in gws {
                    let cleaned = gw_name.split('/').last().unwrap_or(gw_name);
                    let gw_full_key = format!("gw:{}/{}", vs_ns, cleaned);
                    if let Some(&gw_idx) = node_indices.get(&gw_full_key) {
                        graph.add_edge(gw_idx, vs_idx, "routes");
                    }
                }
            }

            // Связываем с сервисами и ServiceEntry
            if let Some(http) = &vs.spec.http {
                for route in http {
                    if let Some(destinations) = &route.route {
                        for dest in destinations {
                            let host = &dest.destination.host;

                            if snapshot.external_hosts.contains(host) {
                                let se_key = format!("se:{}", host);
                                let se_idx = *node_indices
                                    .entry(se_key)
                                    .or_insert_with(|| graph.add_node(TopologyNode::ServiceEntry(host.clone())));
                                graph.add_edge(vs_idx, se_idx, "external");
                            } else {
                                let svc_name = host.split('.').next().unwrap_or(host);
                                let svc_res_key = ResourceKey::new(&vs_ns, svc_name);
                                let ready = snapshot.service_ready_endpoints.get(&svc_res_key).copied().unwrap_or(0);
                                let svc_key = format!("svc:{}/{}", vs_ns, svc_name);

                                let svc_idx = *node_indices.entry(svc_key.clone()).or_insert_with(|| {
                                    graph.add_node(TopologyNode::Service {
                                        name: svc_name.to_string(),
                                        ready_endpoints: ready,
                                    })
                                });
                                graph.add_edge(vs_idx, svc_idx, "forwards");
                            }
                        }
                    }
                }
            }
        }

        // 3. DestinationRules
        for dr in &snapshot.destination_rules {
            let dr_ns = dr.namespace().unwrap_or_default();
            if filter_ns.is_some() && dr.namespace().as_deref() != filter_ns {
                continue;
            }
            let svc_name = dr.spec.host.split('.').next().unwrap_or(&dr.spec.host);
            let svc_key = format!("svc:{}/{}", dr_ns, svc_name);

            let subsets = dr
                .spec
                .subsets
                .as_ref()
                .map(|s| s.iter().map(|sub| sub.name.clone()).collect())
                .unwrap_or_default();

            let dr_idx = graph.add_node(TopologyNode::DestinationRule {
                name: dr.name_any(),
                subsets,
            });

            if let Some(&svc_idx) = node_indices.get(&svc_key) {
                graph.add_edge(svc_idx, dr_idx, "policy");
            }
        }

        Self { graph }
    }

    pub fn to_display_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for node_idx in self.graph.node_indices() {
            if matches!(self.graph[node_idx], TopologyNode::Gateway(_)) {
                self.dfs_format(node_idx, 0, &mut lines, &mut Vec::new());
            }
        }
        if lines.is_empty() {
            lines.push("Маршруты не найдены для выбранного фильтра namespace.".into());
        }
        lines
    }

    fn dfs_format(
        &self,
        curr: NodeIndex,
        depth: usize,
        lines: &mut Vec<String>,
        visited: &mut Vec<NodeIndex>,
    ) {
        if visited.contains(&curr) {
            lines.push(format!("{}└── 🔄 [Обнаружен цикл в роутинге]", "   ".repeat(depth)));
            return;
        }

        visited.push(curr);
        let prefix = if depth == 0 { "" } else { "└── " };
        lines.push(format!("{}{}{}", "   ".repeat(depth), prefix, self.graph[curr].label()));

        for neighbor in self.graph.neighbors(curr) {
            self.dfs_format(neighbor, depth + 1, lines, visited);
        }
        visited.pop();
    }
}