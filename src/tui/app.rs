use crate::analyzer::AuditIssue;
use crate::diagnostics::{
    tetris::NodeTetrisProfile,
    timeline::CausalAnalysis,
    webhooks::WebhookAuditReport,
};
use crate::graph::RouteTrace;
use ratatui::widgets::{ListState, TableState};

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    Duplicates,
    Orphans,
    TrafficGraph,
    IncidentTimeline,
    NodeTetris,
    WebhookAuditor,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum InputMode {
    Normal,
    Searching,
    NamespaceSelect,
}

pub struct AppState {
    pub active_tab: ActiveTab,
    pub input_mode: InputMode,
    pub duplicates: Vec<AuditIssue>,
    pub orphans: Vec<AuditIssue>,
    pub traces: Vec<RouteTrace>,
    pub selected_trace_index: usize,
    pub table_state: TableState,

    // Состояние скролла для графа трафика
    pub route_list_state: ListState,
    pub trace_detail_scroll: u16,

    // Диагностические данные (Kube-Forensics)
    pub incidents: Vec<CausalAnalysis>,
    pub node_profiles: Vec<NodeTetrisProfile>,
    pub webhook_reports: Vec<WebhookAuditReport>,

    pub should_quit: bool,
    pub search_query: String,
    pub namespaces: Vec<String>,
    pub selected_namespace: Option<String>,
    pub ns_selector_index: usize,
    pub show_details: bool,
}

impl AppState {
    pub fn new(
        duplicates: Vec<AuditIssue>,
        orphans: Vec<AuditIssue>,
        traces: Vec<RouteTrace>,
        namespaces: Vec<String>,
        incidents: Vec<CausalAnalysis>,
        node_profiles: Vec<NodeTetrisProfile>,
        webhook_reports: Vec<WebhookAuditReport>,
    ) -> Self {
        let mut table_state = TableState::default();
        if !duplicates.is_empty() {
            table_state.select(Some(0));
        }

        let mut route_list_state = ListState::default();
        if !traces.is_empty() {
            route_list_state.select(Some(0));
        }

        Self {
            active_tab: ActiveTab::Duplicates,
            input_mode: InputMode::Normal,
            duplicates,
            orphans,
            traces,
            selected_trace_index: 0,
            table_state,
            route_list_state,
            trace_detail_scroll: 0,
            incidents,
            node_profiles,
            webhook_reports,
            should_quit: false,
            search_query: String::new(),
            namespaces,
            selected_namespace: None,
            ns_selector_index: 0,
            show_details: false,
        }
    }

    pub fn filtered_issues(&self, is_duplicates: bool) -> Vec<&AuditIssue> {
        let source = if is_duplicates {
            &self.duplicates
        } else {
            &self.orphans
        };

        source
            .iter()
            .filter(|issue| {
                if let Some(ref ns) = self.selected_namespace {
                    if &issue.namespace != ns {
                        return false;
                    }
                }
                if !self.search_query.is_empty() {
                    let q = self.search_query.to_lowercase();
                    return issue.resource.to_lowercase().contains(&q)
                        || issue.namespace.to_lowercase().contains(&q)
                        || issue.description.to_lowercase().contains(&q);
                }
                true
            })
            .collect()
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Duplicates => ActiveTab::Orphans,
            ActiveTab::Orphans => ActiveTab::TrafficGraph,
            ActiveTab::TrafficGraph => ActiveTab::IncidentTimeline,
            ActiveTab::IncidentTimeline => ActiveTab::NodeTetris,
            ActiveTab::NodeTetris => ActiveTab::WebhookAuditor,
            ActiveTab::WebhookAuditor => ActiveTab::Duplicates,
        };
        self.reset_selection();
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Duplicates => ActiveTab::WebhookAuditor,
            ActiveTab::Orphans => ActiveTab::Duplicates,
            ActiveTab::TrafficGraph => ActiveTab::Orphans,
            ActiveTab::IncidentTimeline => ActiveTab::TrafficGraph,
            ActiveTab::NodeTetris => ActiveTab::IncidentTimeline,
            ActiveTab::WebhookAuditor => ActiveTab::NodeTetris,
        };
        self.reset_selection();
    }

    pub fn reset_selection(&mut self) {
        let count = match self.active_tab {
            ActiveTab::Duplicates => self.filtered_issues(true).len(),
            ActiveTab::Orphans => self.filtered_issues(false).len(),
            ActiveTab::TrafficGraph => self.traces.len(),
            ActiveTab::IncidentTimeline => self.incidents.len(),
            ActiveTab::NodeTetris => self.node_profiles.len(),
            ActiveTab::WebhookAuditor => self.webhook_reports.len(),
        };

        if count > 0 {
            self.table_state.select(Some(0));
            self.route_list_state.select(Some(0));
        } else {
            self.table_state.select(None);
            self.route_list_state.select(None);
        }
        self.selected_trace_index = 0;
        self.trace_detail_scroll = 0;
    }

    pub fn next_item(&mut self) {
        if self.input_mode == InputMode::NamespaceSelect {
            if self.ns_selector_index < self.namespaces.len() {
                self.ns_selector_index += 1;
            }
            return;
        }

        if self.active_tab == ActiveTab::TrafficGraph {
            if !self.traces.is_empty() {
                self.selected_trace_index = (self.selected_trace_index + 1) % self.traces.len();
                self.route_list_state.select(Some(self.selected_trace_index));
                self.trace_detail_scroll = 0;
            }
            return;
        }

        let count = match self.active_tab {
            ActiveTab::Duplicates => self.filtered_issues(true).len(),
            ActiveTab::Orphans => self.filtered_issues(false).len(),
            ActiveTab::IncidentTimeline => self.incidents.len(),
            ActiveTab::NodeTetris => self.node_profiles.len(),
            ActiveTab::WebhookAuditor => self.webhook_reports.len(),
            ActiveTab::TrafficGraph => 0,
        };

        if count > 0 {
            let i = match self.table_state.selected() {
                Some(i) => (i + 1) % count,
                None => 0,
            };
            self.table_state.select(Some(i));
        }
    }

    pub fn previous_item(&mut self) {
        if self.input_mode == InputMode::NamespaceSelect {
            self.ns_selector_index = self.ns_selector_index.saturating_sub(1);
            return;
        }

        if self.active_tab == ActiveTab::TrafficGraph {
            if !self.traces.is_empty() {
                self.selected_trace_index = (self.selected_trace_index + self.traces.len() - 1) % self.traces.len();
                self.route_list_state.select(Some(self.selected_trace_index));
                self.trace_detail_scroll = 0;
            }
            return;
        }

        let count = match self.active_tab {
            ActiveTab::Duplicates => self.filtered_issues(true).len(),
            ActiveTab::Orphans => self.filtered_issues(false).len(),
            ActiveTab::IncidentTimeline => self.incidents.len(),
            ActiveTab::NodeTetris => self.node_profiles.len(),
            ActiveTab::WebhookAuditor => self.webhook_reports.len(),
            ActiveTab::TrafficGraph => 0,
        };

        if count > 0 {
            let i = match self.table_state.selected() {
                Some(i) => (i + count - 1) % count,
                None => 0,
            };
            self.table_state.select(Some(i));
        }
    }

    pub fn scroll_detail_down(&mut self) {
        self.trace_detail_scroll = self.trace_detail_scroll.saturating_add(2);
    }

    pub fn scroll_detail_up(&mut self) {
        self.trace_detail_scroll = self.trace_detail_scroll.saturating_sub(2);
    }
}