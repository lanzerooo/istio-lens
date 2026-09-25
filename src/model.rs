use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Безопасная десериализация: обрабатывает как отсутствие поля, так и явный `null`
pub fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "networking.istio.io",
    version = "v1beta1",
    kind = "Gateway",
    plural = "gateways",
    namespaced
)]
pub struct GatewaySpec {
    pub selector: Option<BTreeMap<String, String>>,
    pub servers: Option<Vec<Server>>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct Server {
    pub port: Option<Port>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
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
    #[serde(default)]
    pub host: String,
    pub subsets: Option<Vec<Subset>>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
pub struct Subset {
    pub name: String,
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(CustomResource, Serialize, Deserialize, Default, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "networking.istio.io",
    version = "v1beta1",
    kind = "ServiceEntry",
    plural = "serviceentries",
    namespaced
)]
pub struct ServiceEntrySpec {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub hosts: Vec<String>,
    pub location: Option<String>,
    pub resolution: Option<String>,
}

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
}