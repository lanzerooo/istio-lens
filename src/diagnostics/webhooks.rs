use k8s_openapi::api::admissionregistration::v1::{
    MutatingWebhookConfiguration, ValidatingWebhookConfiguration,
};
use kube::ResourceExt;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookRiskLevel {
    Safe,
    Warning,
    ClusterBlocker, // Если сервис упадет, создание ресурсов в кластере будет полностью заблокировано!
}

#[derive(Debug, Clone)]
pub struct WebhookAuditReport {
    pub name: String,
    pub hook_type: &'static str, // Mutating / Validating
    pub service_ref: String,     // namespace/service-name:port
    pub failure_policy: String,  // Fail / Ignore
    pub timeout_seconds: i32,
    pub intercepts_all_pods: bool,
    pub service_has_endpoints: bool,
    pub risk_level: WebhookRiskLevel,
    pub risk_explanation: String,
}

pub struct WebhookAuditor;

impl WebhookAuditor {
    pub fn audit(
        mutating: &[MutatingWebhookConfiguration],
        validating: &[ValidatingWebhookConfiguration],
        active_service_endpoints: &HashSet<String>, // "namespace/name"
    ) -> Vec<WebhookAuditReport> {
        let mut reports = Vec::new();

        // 1. Аудит Mutating Webhooks
        for m in mutating {
            let config_name = m.name_any();
            if let Some(webhooks) = &m.webhooks {
                for hook in webhooks {
                    let (svc_ref, svc_key) = extract_service_ref(&hook.client_config.service);
                    let failure_policy = hook.failure_policy.clone().unwrap_or_else(|| "Fail".into());
                    let timeout = hook.timeout_seconds.unwrap_or(10);

                    let intercepts_all = hook.rules.as_ref().map_or(false, |rules| {
                        rules.iter().any(|r| {
                            let matches_pods = r.resources.as_deref().unwrap_or_default().iter().any(|res| res == "pods" || res == "*");
                            let matches_create = r.operations.as_deref().unwrap_or_default().iter().any(|op| op == "CREATE" || op == "*");
                            matches_pods && matches_create
                        })
                    });

                    let has_endpoints = active_service_endpoints.contains(&svc_key);

                    let (risk_level, explanation) = assess_risk(
                        &failure_policy,
                        intercepts_all,
                        has_endpoints,
                        timeout,
                    );

                    reports.push(WebhookAuditReport {
                        name: format!("{}/{}", config_name, hook.name),
                        hook_type: "Mutating",
                        service_ref: svc_ref,
                        failure_policy,
                        timeout_seconds: timeout,
                        intercepts_all_pods: intercepts_all,
                        service_has_endpoints: has_endpoints,
                        risk_level,
                        risk_explanation: explanation,
                    });
                }
            }
        }

        // 2. Аудит Validating Webhooks
        for v in validating {
            let config_name = v.name_any();
            if let Some(webhooks) = &v.webhooks {
                for hook in webhooks {
                    let (svc_ref, svc_key) = extract_service_ref(&hook.client_config.service);
                    let failure_policy = hook.failure_policy.clone().unwrap_or_else(|| "Fail".into());
                    let timeout = hook.timeout_seconds.unwrap_or(10);

                    let intercepts_all = hook.rules.as_ref().map_or(false, |rules| {
                        rules.iter().any(|r| {
                            let matches_pods = r.resources.as_deref().unwrap_or_default().iter().any(|res| res == "pods" || res == "*");
                            let matches_create = r.operations.as_deref().unwrap_or_default().iter().any(|op| op == "CREATE" || op == "*");
                            matches_pods && matches_create
                        })
                    });

                    let has_endpoints = active_service_endpoints.contains(&svc_key);

                    let (risk_level, explanation) = assess_risk(
                        &failure_policy,
                        intercepts_all,
                        has_endpoints,
                        timeout,
                    );

                    reports.push(WebhookAuditReport {
                        name: format!("{}/{}", config_name, hook.name),
                        hook_type: "Validating",
                        service_ref: svc_ref,
                        failure_policy,
                        timeout_seconds: timeout,
                        intercepts_all_pods: intercepts_all,
                        service_has_endpoints: has_endpoints,
                        risk_level,
                        risk_explanation: explanation,
                    });
                }
            }
        }

        reports
    }
}

fn extract_service_ref(
    svc: &Option<k8s_openapi::api::admissionregistration::v1::ServiceReference>,
) -> (String, String) {
    if let Some(s) = svc {
        let port = s.port.unwrap_or(443);
        let key = format!("{}/{}", s.namespace, s.name);
        (format!("{}:{}", key, port), key)
    } else {
        ("External URL".into(), "".into())
    }
}

fn assess_risk(
    failure_policy: &str,
    intercepts_all_pods: bool,
    has_endpoints: bool,
    timeout: i32,
) -> (WebhookRiskLevel, String) {
    if failure_policy == "Fail" && !has_endpoints {
        (
            WebhookRiskLevel::ClusterBlocker,
            "CATASTROPHIC RISK: failurePolicy: Fail, но у сервиса вебхука 0 готовых подов! Создание подов в кластере парализовано.".into(),
        )
    } else if failure_policy == "Fail" && intercepts_all_pods && timeout >= 10 {
        (
            WebhookRiskLevel::Warning,
            format!("ВЫСОКИЙ РИСК: Перехватывает создание всех подов с таймаутом {}s. Падение бэкенда вебхука заблокирует деплои.", timeout),
        )
    } else {
        (WebhookRiskLevel::Safe, "Штатный режим: рисков блокировки API не обнаружено.".into())
    }
}