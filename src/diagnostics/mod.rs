pub mod tetris;
pub mod timeline;
pub mod webhooks;

use k8s_openapi::api::admissionregistration::v1::{
    MutatingWebhookConfiguration, ValidatingWebhookConfiguration,
};
use k8s_openapi::api::core::v1::{Event, Node, Pod};
use kube::{api::ListParams, Api, Client, ResourceExt};
use std::collections::HashSet;

pub struct DiagnosticsSnapshot {
    pub incidents: Vec<timeline::CausalAnalysis>,
    pub node_profiles: Vec<tetris::NodeTetrisProfile>,
    pub webhook_reports: Vec<webhooks::WebhookAuditReport>,
}

pub struct DiagnosticsCollector {
    client: Client,
}

impl DiagnosticsCollector {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn collect(&self) -> Result<DiagnosticsSnapshot, crate::error::AppError> {
        let pod_api: Api<Pod> = Api::all(self.client.clone());
        let event_api: Api<Event> = Api::all(self.client.clone());
        let node_api: Api<Node> = Api::all(self.client.clone());
        let m_webhook_api: Api<MutatingWebhookConfiguration> = Api::all(self.client.clone());
        let v_webhook_api: Api<ValidatingWebhookConfiguration> = Api::all(self.client.clone());

        let lp = ListParams::default();

        let (pods, events, nodes, m_webhooks, v_webhooks) = tokio::try_join!(
            pod_api.list(&lp),
            event_api.list(&lp),
            node_api.list(&lp),
            m_webhook_api.list(&lp),
            v_webhook_api.list(&lp),
        )?;

        // 1. Поиск инцидентов и построение таймлайнов
        let mut incidents = Vec::new();
        for pod in &pods.items {
            if let Some(analysis) = timeline::IncidentTimelineEngine::analyze_pod(pod, &events.items) {
                incidents.push(analysis);
            }
        }

        // 2. Построение профилей Tetris по нодам
        let node_profiles = tetris::TetrisAnalyzer::build_profiles(&nodes.items, &pods.items);

        // 3. Сбор активных сервисов для проверки вебхуков
        let mut active_services = HashSet::new();
        for pod in &pods.items {
            let phase = pod.status.as_ref().and_then(|s| s.phase.as_deref());
            if phase == Some("Running") {
                if let Some(ns) = pod.namespace() {
                    // Используем приблизительное сопоставление app-лейблов
                    if let Some(app) = pod.metadata.labels.as_ref().and_then(|l| l.get("app")) {
                        active_services.insert(format!("{}/{}", ns, app));
                    }
                }
            }
        }

        let webhook_reports = webhooks::WebhookAuditor::audit(
            &m_webhooks.items,
            &v_webhooks.items,
            &active_services,
        );

        Ok(DiagnosticsSnapshot {
            incidents,
            node_profiles,
            webhook_reports,
        })
    }
}