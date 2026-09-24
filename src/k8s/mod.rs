use crate::error::AppError;
use crate::model::{DestinationRule, Gateway, ResourceKey, ServiceEntry, VirtualService};
use k8s_openapi::api::core::v1::{Endpoints, Service};
use kube::{api::ListParams, Api, Client, ResourceExt};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub struct ClusterSnapshot {
    pub gateways: Vec<Gateway>,
    pub virtual_services: Vec<VirtualService>,
    pub destination_rules: Vec<DestinationRule>,
    pub service_entries: Vec<ServiceEntry>,
    pub existing_service_keys: HashSet<ResourceKey>,
    pub external_hosts: HashSet<String>,
    /// Карта: ResourceKey сервиса -> количество готовых подов (Ready Endpoints)
    pub service_ready_endpoints: HashMap<ResourceKey, usize>,
    pub namespaces: Vec<String>,
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
        let se_api: Api<ServiceEntry> = Api::all(self.client.clone());
        let svc_api: Api<Service> = Api::all(self.client.clone());
        let ep_api: Api<Endpoints> = Api::all(self.client.clone());

        let lp = ListParams::default();

        // 6 параллельных запросов без взаимных блокировок
        let (gateways, virtual_services, destination_rules, service_entries, services, endpoints) = tokio::try_join!(
            gw_api.list(&lp),
            vs_api.list(&lp),
            dr_api.list(&lp),
            se_api.list(&lp),
            svc_api.list(&lp),
            ep_api.list(&lp)
        )?;

        let mut existing_service_keys = HashSet::new();
        let mut namespaces_set = HashSet::new();

        for svc in &services {
            if let Some(ns) = svc.namespace() {
                existing_service_keys.insert(ResourceKey::new(&ns, svc.name_any()));
                namespaces_set.insert(ns);
            }
        }

        // Индексация внешних хостов из ServiceEntry
        let mut external_hosts = HashSet::new();
        for se in &service_entries {
            for host in &se.spec.hosts {
                external_hosts.insert(host.clone());
            }
            if let Some(ns) = se.namespace() {
                namespaces_set.insert(ns);
            }
        }

        // Подсчет готовых эндпоинтов (Ready pods)
        let mut service_ready_endpoints = HashMap::new();
        for ep in endpoints {
            if let (Some(ns), name) = (ep.namespace(), ep.name_any()) {
                let ready_count: usize = ep
                    .subsets
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(|s| s.addresses.as_deref().unwrap_or_default().len())
                    .sum();
                service_ready_endpoints.insert(ResourceKey::new(ns, name), ready_count);
            }
        }

        let mut namespaces: Vec<String> = namespaces_set.into_iter().collect();
        namespaces.sort();

        Ok(ClusterSnapshot {
            gateways: gateways.items,
            virtual_services: virtual_services.items,
            destination_rules: destination_rules.items,
            service_entries: service_entries.items,
            existing_service_keys,
            external_hosts,
            service_ready_endpoints,
            namespaces,
        })
    }
}