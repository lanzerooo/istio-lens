use k8s_openapi::api::core::v1::{Node, Pod};
use kube::ResourceExt;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ResourceAllocation {
    pub allocatable: i64,
    pub requests: i64,
    pub limits: i64,
}

impl ResourceAllocation {
    pub fn requests_pct(&self) -> f64 {
        if self.allocatable == 0 { 0.0 } else { (self.requests as f64 / self.allocatable as f64) * 100.0 }
    }

    pub fn limits_pct(&self) -> f64 {
        if self.allocatable == 0 { 0.0 } else { (self.limits as f64 / self.allocatable as f64) * 100.0 }
    }

    /// Риск взрыва ноды: во сколько раз Limits превышают физический объем
    pub fn hazard_ratio(&self) -> f64 {
        if self.allocatable == 0 { 0.0 } else { self.limits as f64 / self.allocatable as f64 }
    }
}

#[derive(Debug, Clone)]
pub struct NodeTetrisProfile {
    pub node_name: String,
    pub cpu: ResourceAllocation,
    pub memory: ResourceAllocation,
    pub best_effort_pods: Vec<String>,
    pub burstable_pods: Vec<String>,
    pub guaranteed_pods: Vec<String>,
}

pub struct TetrisAnalyzer;

impl TetrisAnalyzer {
    pub fn build_profiles(nodes: &[Node], pods: &[Pod]) -> Vec<NodeTetrisProfile> {
        let mut profiles = Vec::new();

        for node in nodes {
            let node_name = node.name_any();

            // Вычисляем Allocatable емкость
            let alloc_cpu = node
                .status
                .as_ref()
                .and_then(|s| s.allocatable.as_ref())
                .and_then(|a| a.get("cpu"))
                .map(|q| parse_cpu_quantity(&q.0))
                .unwrap_or(0);

            let alloc_mem = node
                .status
                .as_ref()
                .and_then(|s| s.allocatable.as_ref())
                .and_then(|a| a.get("memory"))
                .map(|q| parse_memory_quantity(&q.0))
                .unwrap_or(0);

            let mut req_cpu = 0;
            let mut lim_cpu = 0;
            let mut req_mem = 0;
            let mut lim_mem = 0;

            let mut best_effort = Vec::new();
            let mut burstable = Vec::new();
            let mut guaranteed = Vec::new();

            // Фильтруем поды, привязанные к данной ноде и не находящиеся в статусе Succeeded/Failed
            for pod in pods {
                if pod.spec.as_ref().and_then(|s| s.node_name.as_deref()) != Some(&node_name) {
                    continue;
                }

                let phase = pod.status.as_ref().and_then(|s| s.phase.as_deref()).unwrap_or("Pending");
                if phase == "Succeeded" || phase == "Failed" {
                    continue;
                }

                let (p_req_cpu, p_lim_cpu, p_req_mem, p_lim_mem, qos) = extract_pod_resources(pod);

                req_cpu += p_req_cpu;
                lim_cpu += p_lim_cpu;
                req_mem += p_req_mem;
                lim_mem += p_lim_mem;

                let name = format!("{}/{}", pod.namespace().unwrap_or_default(), pod.name_any());
                match qos {
                    QosClass::Guaranteed => guaranteed.push(name),
                    QosClass::Burstable => burstable.push(name),
                    QosClass::BestEffort => best_effort.push(name),
                }
            }

            profiles.push(NodeTetrisProfile {
                node_name,
                cpu: ResourceAllocation {
                    allocatable: alloc_cpu,
                    requests: req_cpu,
                    limits: lim_cpu,
                },
                memory: ResourceAllocation {
                    allocatable: alloc_mem,
                    requests: req_mem,
                    limits: lim_mem,
                },
                best_effort_pods: best_effort,
                burstable_pods: burstable,
                guaranteed_pods: guaranteed,
            });
        }

        profiles
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QosClass {
    Guaranteed,
    Burstable,
    BestEffort,
}

fn extract_pod_resources(pod: &Pod) -> (i64, i64, i64, i64, QosClass) {
    let mut req_cpu = 0;
    let mut lim_cpu = 0;
    let mut req_mem = 0;
    let mut lim_mem = 0;

    let mut has_req = false;
    let mut has_lim = false;
    let mut all_match = true;

    if let Some(spec) = &pod.spec {
        for c in &spec.containers {
            if let Some(resources) = &c.resources {
                if let Some(reqs) = &resources.requests {
                    if let Some(cpu) = reqs.get("cpu") { req_cpu += parse_cpu_quantity(&cpu.0); has_req = true; }
                    if let Some(mem) = reqs.get("memory") { req_mem += parse_memory_quantity(&mem.0); has_req = true; }
                }
                if let Some(lims) = &resources.limits {
                    if let Some(cpu) = lims.get("cpu") { lim_cpu += parse_cpu_quantity(&cpu.0); has_lim = true; }
                    if let Some(mem) = lims.get("memory") { lim_mem += parse_memory_quantity(&mem.0); has_lim = true; }
                }
            }
        }
    }

    if req_cpu != lim_cpu || req_mem != lim_mem || !has_lim {
        all_match = false;
    }

    let qos = if !has_req && !has_lim {
        QosClass::BestEffort
    } else if all_match {
        QosClass::Guaranteed
    } else {
        QosClass::Burstable
    };

    (req_cpu, lim_cpu, req_mem, lim_mem, qos)
}

/// Конвертация единиц CPU в millicores (1 core = 1000m)
fn parse_cpu_quantity(q: &str) -> i64 {
    if let Some(stripped) = q.strip_suffix('m') {
        stripped.parse::<i64>().unwrap_or(0)
    } else {
        (q.parse::<f64>().unwrap_or(0.0) * 1000.0) as i64
    }
}

/// Конвертация единиц памяти в мегабайты (MiB)
fn parse_memory_quantity(q: &str) -> i64 {
    let (num, mult) = if let Some(s) = q.strip_suffix("Ki") {
        (s, 1.0 / 1024.0)
    } else if let Some(s) = q.strip_suffix("Mi") {
        (s, 1.0)
    } else if let Some(s) = q.strip_suffix("Gi") {
        (s, 1024.0)
    } else if let Some(s) = q.strip_suffix("Ti") {
        (s, 1024.0 * 1024.0)
    } else {
        (q, 1.0 / (1024.0 * 1024.0)) // в байтах
    };
    (num.parse::<f64>().unwrap_or(0.0) * mult) as i64
}