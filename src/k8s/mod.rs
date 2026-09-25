use crate::error::AppError;
use crate::model::{DestinationRule, Gateway, ResourceKey, ServiceEntry, VirtualService};
use k8s_openapi::api::core::v1::{Pod, Service};
use kube::{api::ListParams, Api, Client, ResourceExt};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub struct PodInfo {
    pub name: String,
    pub namespace: String,
    pub labels: BTreeMap<String, String>,
    pub is_ready: bool,
    pub phase: String,
}

#[derive(Clone, Debug, Default)]
pub struct ServiceMeta {
    pub name: String,
    pub namespace: String,
    pub selector: BTreeMap<String, String>,
    pub ports: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ClusterSnapshot {
    pub gateways: Vec<Gateway>,
    pub virtual_services: Vec<VirtualService>,
    pub destination_rules: Vec<DestinationRule>,
    pub service_entries: Vec<ServiceEntry>,
    pub existing_service_keys: HashSet<ResourceKey>,
    pub external_hosts: HashSet<String>,
    pub services_meta: HashMap<ResourceKey, ServiceMeta>,
    pub pods: Vec<PodInfo>,
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
        let pod_api: Api<Pod> = Api::all(self.client.clone());

        let lp = ListParams::default();

        // 6 параллельных неблокирующих вызовов
        let (gateways, virtual_services, destination_rules, service_entries, services, pods) = tokio::try_join!(
            gw_api.list(&lp),
            vs_api.list(&lp),
            dr_api.list(&lp),
            se_api.list(&lp),
            svc_api.list(&lp),
            pod_api.list(&lp)
        )?;

        let mut existing_service_keys = HashSet::new();
        let mut services_meta = HashMap::new();
        let mut namespaces_set = HashSet::new();

        for svc in services.items {
            if let Some(ns) = svc.namespace() {
                let name = svc.name_any();
                let key = ResourceKey::new(&ns, &name);
                existing_service_keys.insert(key.clone());
                namespaces_set.insert(ns.clone());

                let selector = svc
                    .spec
                    .as_ref()
                    .and_then(|s| s.selector.clone())
                    .map(|m| m.into_iter().collect())
                    .unwrap_or_default();

                let ports = svc
                    .spec
                    .as_ref()
                    .and_then(|s| s.ports.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| format!("{}:{}", p.name.unwrap_or_default(), p.port))
                    .collect();

                services_meta.insert(
                    key,
                    ServiceMeta {
                        name,
                        namespace: ns,
                        selector,
                        ports,
                    },
                );
            }
        }

        // Внешние хосты из ServiceEntry
        let mut external_hosts = HashSet::new();
        for se in &service_entries.items {
            for host in &se.spec.hosts {
                external_hosts.insert(host.clone());
            }
            if let Some(ns) = se.namespace() {
                namespaces_set.insert(ns);
            }
        }

        // Индексация подов и проверка условий контейнеров (Ready)
        let mut parsed_pods = Vec::new();
        for p in pods.items {
            let ns = p.namespace().unwrap_or_else(|| "default".into());
            let name = p.name_any();
            let labels = p
                .metadata
                .labels
                .clone()
                .map(|m| m.into_iter().collect())
                .unwrap_or_default();

            let phase = p
                .status
                .as_ref()
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".into());

            let is_ready = p
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref())
                .map(|statuses| statuses.iter().all(|c| c.ready))
                .unwrap_or(false)
                && phase == "Running";

            parsed_pods.push(PodInfo {
                name,
                namespace: ns,
                labels,
                is_ready,
                phase,
            });
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
            services_meta,
            pods: parsed_pods,
            namespaces,
        })
    }
}