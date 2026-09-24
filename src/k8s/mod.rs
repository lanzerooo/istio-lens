use crate::error::AppError;
use crate::model::{DestinationRule, Gateway, ResourceKey, VirtualService};
use k8s_openapi::api::core::v1::Service;
use kube::{api::ListParams, Api, Client, ResourceExt};
use std::collections::HashSet;

/// Снимок состояния сетевого стека кластера
#[derive(Clone, Debug, Default)]
pub struct ClusterSnapshot {
    pub gateways: Vec<Gateway>,
    pub virtual_services: Vec<VirtualService>,
    pub destination_rules: Vec<DestinationRule>,
    pub existing_service_keys: HashSet<ResourceKey>,
}

pub struct K8sDiscovery {
    client: Client,
}

impl K8sDiscovery {
    pub async fn new() -> Result<Self, AppError> {
        let client = Client::try_default().await?;
        Ok(Self { client })
    }

    pub async fn fetch_snapshot(&self) -> Result<ClusterSnapshot, AppError> {
        let gw_api: Api<Gateway> = Api::all(self.client.clone());
        let vs_api: Api<VirtualService> = Api::all(self.client.clone());
        let dr_api: Api<DestinationRule> = Api::all(self.client.clone());
        let svc_api: Api<Service> = Api::all(self.client.clone());

        let lp = ListParams::default();

        // Параллельный сбор ресурсов без взаимных блокировок
        let (gateways, virtual_services, destination_rules, services) = tokio::try_join!(
            gw_api.list(&lp),
            vs_api.list(&lp),
            dr_api.list(&lp),
            svc_api.list(&lp)
        )?;

        // Извлекаем ключи по значению, избегая лишнего клонирования
        let mut existing_service_keys = HashSet::new();
        for svc in services.items {
            if let Some(ns) = svc.namespace() {
                existing_service_keys.insert(ResourceKey::new(ns, svc.name_any()));
            }
        }

        Ok(ClusterSnapshot {
            gateways: gateways.items,
            virtual_services: virtual_services.items,
            destination_rules: destination_rules.items,
            existing_service_keys,
        })
    }
}