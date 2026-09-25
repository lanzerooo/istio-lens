use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::{Event, Pod};
use kube::ResourceExt;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncidentSeverity {
    Info,
    Warning,
    Fatal,
}

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub timestamp: DateTime<Utc>,
    pub source: String,       // Node, Kubelet, OOM-Killer, HPA, Container
    pub reason: String,       // FailedScheduling, BackOff, OOMKilled, Killing
    pub message: String,
    pub severity: IncidentSeverity,
}

#[derive(Debug, Clone)]
pub struct CausalAnalysis {
    pub pod_name: String,
    pub namespace: String,
    pub exit_code: Option<i32>,
    pub root_cause_verdict: String,
    pub is_node_level_failure: bool,
    pub events: Vec<TimelineEntry>,
}

pub struct IncidentTimelineEngine;

impl IncidentTimelineEngine {
    pub fn analyze_pod(pod: &Pod, cluster_events: &[Event]) -> Option<CausalAnalysis> {
        let pod_name = pod.name_any();
        let namespace = pod.namespace().unwrap_or_else(|| "default".into());
        let node_name = pod.spec.as_ref().and_then(|s| s.node_name.clone());

        // 1. Проверяем статус последнего завершения контейнеров
        let mut exit_code = None;
        let mut termination_reason = None;

        if let Some(status) = &pod.status {
            if let Some(container_statuses) = &status.container_statuses {
                for cs in container_statuses {
                    if let Some(state) = &cs.last_state {
                        if let Some(terminated) = &state.terminated {
                            exit_code = Some(terminated.exit_code);
                            termination_reason = Some(terminated.reason.clone().unwrap_or_default());
                        }
                    }
                }
            }
        }

        // Если под никогда не падал и работает штатно — пропускаем
        if exit_code.is_none() && pod.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Running") {
            return None;
        }

        // 2. Сбор и корреляция событий: связанные с подом И связанные с нодой
        let mut timeline: Vec<TimelineEntry> = Vec::new();

        for ev in cluster_events {
            let matches_pod = ev.involved_object.kind.as_deref() == Some("Pod")
                && ev.involved_object.name.as_deref() == Some(&pod_name)
                && ev.involved_object.namespace.as_deref() == Some(&namespace);

            let matches_node = node_name.is_some()
                && ev.involved_object.kind.as_deref() == Some("Node")
                && ev.involved_object.name == node_name;

            if matches_pod || matches_node {
                let ts = ev
                    .last_timestamp
                    .as_ref()
                    .map(|t| t.0)
                    .or_else(|| ev.event_time.as_ref().map(|t| t.0))
                    .unwrap_or_else(Utc::now);

                let reason = ev.reason.clone().unwrap_or_else(|| "Unknown".into());
                let message = ev.message.clone().unwrap_or_default();
                let is_warning = ev.type_.as_deref() == Some("Warning");

                let severity = if reason.contains("OOM") || reason.contains("Evict") || reason.contains("Failed") {
                    IncidentSeverity::Fatal
                } else if is_warning {
                    IncidentSeverity::Warning
                } else {
                    IncidentSeverity::Info
                };

                let source = if matches_node {
                    format!("Node: {}", node_name.as_deref().unwrap_or_default())
                } else {
                    ev.source.as_ref().and_then(|s| s.component.clone()).unwrap_or_else(|| "Kubelet".into())
                };

                timeline.push(TimelineEntry {
                    timestamp: ts,
                    source,
                    reason,
                    message,
                    severity,
                });
            }
        }

        // Сортировка по времени
        timeline.sort_by_key(|e| e.timestamp);

        // 3. Аналитический вердикт первопричины (Root Cause Inference)
        let has_node_memory_pressure = timeline.iter().any(|e| {
            e.source.starts_with("Node") && (e.reason == "MemoryPressure" || e.reason == "NodeHasMemoryPressure")
        });

        let mut is_node_level = false;
        let verdict = match (exit_code, termination_reason.as_deref()) {
            (Some(137), Some("OOMKilled")) if has_node_memory_pressure => {
                is_node_level = true;
                "КРИТИЧЕСКИЙ СБОЙ НОДЫ: Контейнер уничтожен Linux OOM-Killer из-за исчерпания системной памяти всей ноды (Node MemoryPressure). Лимиты пода вторичны.".into()
            }
            (Some(137), Some("OOMKilled")) => {
                "CONTAINER LIMIT OOM: Контейнер превысил жесткий лимит memory.limits. Памяти на ноде было достаточно.".into()
            }
            (Some(137), _) => {
                "SIGKILL (137): Процесс принудительно убит извне (Liveness Probe timeout или отказ Kubelet Grace Period).".into()
            }
            (Some(143), _) => {
                "SIGTERM (143): Корректное завершение по инициативе Kubelet (Preemption, ручной drain или ротация ноды).".into()
            }
            (Some(1), _) | (Some(2), _) => {
                "APPLICATION PANIC/CRASH: Внутренний сбой среды выполнения (Uncaught Exception или ранняя паника main-потока).".into()
            }
            _ => {
                if timeline.iter().any(|e| e.reason == "FailedScheduling") {
                    "SCHEDULER BLOCKED: Невозможно разместить под из-за нехватки CPU/Memory Requests или несовпадения Taints/Tolerations.".into()
                } else {
                    format!("UNEXPECTED EXIT: Под завершился с кодом {:?}", exit_code)
                }
            }
        };

        Some(CausalAnalysis {
            pod_name,
            namespace,
            exit_code,
            root_cause_verdict: verdict,
            is_node_level_failure: is_node_level,
            events: timeline,
        })
    }
}