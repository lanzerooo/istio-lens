use crate::k8s::ClusterSnapshot;
use kube::ResourceExt;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TopologyNode {
    Gateway(String),
    VirtualService(String),
    Service(String),
    DestinationRule { name: String, subsets: Vec<String> },
}

impl TopologyNode {
    pub fn label(&self) -> String {
        match self {
            Self::Gateway(name) => format!("🌐 Gateway [{}]", name),
            Self::VirtualService(name) => format!("🔀 VS [{}]", name),
            Self::Service(name) => format!("⚙️  Svc [{}]", name),
            Self::DestinationRule { name, subsets } => {
                format!("🎯 DR [{}] (subsets: {})", name, subsets.join(", "))
            }
        }
    }
}

pub struct TrafficGraph {
    pub graph: DiGraph<TopologyNode, &'static str>,
}

impl TrafficGraph {
    pub fn build(snapshot: &ClusterSnapshot) -> Self {
        let mut graph = DiGraph::new();
        let mut node_indices: HashMap<String, NodeIndex> = HashMap::new();

        // 1. Добавляем Gateway
        for gw in &snapshot.gateways {
            let key = format!("gw:{}", gw.name_any());
            let idx = graph.add_node(TopologyNode::Gateway(gw.name_any()));
            node_indices.insert(key, idx);
        }

        // 2. Добавляем VirtualServices и связываем с Gateways
        for vs in &snapshot.virtual_services {
            let vs_key = format!("vs:{}", vs.name_any());
            let vs_idx = graph.add_node(TopologyNode::VirtualService(vs.name_any()));
            node_indices.insert(vs_key, vs_idx);

            if let Some(gws) = &vs.spec.gateways {
                for gw_name in gws {
                    let cleaned_gw = gw_name.split('/').last().unwrap_or(gw_name);
                    let gw_key = format!("gw:{}", cleaned_gw);
                    if let Some(&gw_idx) = node_indices.get(&gw_key) {
                        graph.add_edge(gw_idx, vs_idx, "binds");
                    }
                }
            }

            // Связываем VS с целевыми K8s Services
            if let Some(http) = &vs.spec.http {
                for route in http {
                    if let Some(destinations) = &route.route {
                        for dest in destinations {
                            let svc_name = dest.destination.host.split('.').next().unwrap_or(&dest.destination.host);
                            let svc_key = format!("svc:{}", svc_name);

                            let svc_idx = *node_indices
                                .entry(svc_key.clone())
                                .or_insert_with(|| graph.add_node(TopologyNode::Service(svc_name.to_string())));

                            graph.add_edge(vs_idx, svc_idx, "routes");
                        }
                    }
                }
            }
        }

        // 3. Добавляем DestinationRules и связываем с Services
        for dr in &snapshot.destination_rules {
            let svc_name = dr.spec.host.split('.').next().unwrap_or(&dr.spec.host);
            let svc_key = format!("svc:{}", svc_name);

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
                graph.add_edge(svc_idx, dr_idx, "applies");
            }
        }

        Self { graph }
    }

    /// Преобразует граф в плоское иерархическое дерево для отрисовки в TUI
    pub fn to_display_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();

        // Начинаем обход от корней (шлюзов)
        for node_idx in self.graph.node_indices() {
            if matches!(self.graph[node_idx], TopologyNode::Gateway(_)) {
                self.dfs_format(node_idx, 0, &mut lines, &mut Vec::new());
            }
        }

        if lines.is_empty() {
            lines.push("Связи в кластере не обнаружены (нет связанных шлюзов).".into());
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
            lines.push(format!("{}└── 🔄 [Cycle Detected]", "  ".repeat(depth)));
            return;
        }

        visited.push(curr);
        let prefix = if depth == 0 { "" } else { "└── " };
        lines.push(format!("{}{}{}", "  ".repeat(depth), prefix, self.graph[curr].label()));

        for neighbor in self.graph.neighbors(curr) {
            self.dfs_format(neighbor, depth + 1, lines, visited);
        }

        visited.pop();
    }
}