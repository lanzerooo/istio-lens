use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "networking.istio.io",
    version = "v1beta1",
    kind = "Gateway",
    plural = "gateways",
    namespaced
)]
pub struct GatewaySpec {
    pub selector: BTreeMap<String, String>,
    pub servers: Vec<Server>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct Server {
    pub port: Port,
    pub hosts: Vec<String>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct Port {
    pub number: u32,
    pub name: String,
    pub protocol: String,
}

#[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "networking.istio.io",
    version = "v1beta1",
    kind = "VirtualService",
    plural = "virtualservices",
    namespaced
)]
pub struct VirtualServiceSpec {
    pub hosts: Vec<String>,
    pub gateways: Option<Vec<String>>,
    pub http: Option<Vec<HttpRoute>>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct HttpRoute {
    pub name: Option<String>,
    pub r#match: Option<Vec<HttpMatchRequest>>,
    pub route: Option<Vec<HttpRouteDestination>>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct HttpMatchRequest {
    pub uri: Option<StringMatch>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StringMatch {
    Exact(String),
    Prefix(String),
    Regex(String),
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct HttpRouteDestination {
    pub destination: Destination,
    pub weight: Option<i32>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct Destination {
    pub host: String,
    pub subset: Option<String>,
    pub port: Option<PortSelector>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct PortSelector {
    pub number: Option<u32>,
}

#[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "networking.istio.io",
    version = "v1beta1",
    kind = "DestinationRule",
    plural = "destinationrules",
    namespaced
)]
pub struct DestinationRuleSpec {
    pub host: String,
    pub subsets: Option<Vec<Subset>>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct Subset {
    pub name: String,
    pub labels: Option<BTreeMap<String, String>>,
}

/// Унифицированный идентификатор ресурса в кластере
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceKey {
    pub namespace: String,
    pub name: String,
}

impl ResourceKey {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    #[allow(dead_code)]
    pub fn to_fqdn(&self) -> String {
        format!("{}.{}.svc.cluster.local", self.name, self.namespace)
    }
}