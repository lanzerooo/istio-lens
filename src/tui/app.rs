use crate::analyzer::AuditIssue;
use ratatui::widgets::TableState;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ActiveTab {
    Duplicates,
    Orphans,
    TrafficGraph,
}

pub struct AppState {
    pub active_tab: ActiveTab,
    pub duplicates: Vec<AuditIssue>,
    pub orphans: Vec<AuditIssue>,
    pub graph_lines: Vec<String>,
    pub table_state: TableState,
    pub graph_scroll: usize,
    pub should_quit: bool,
}

impl AppState {
    pub fn new(
        duplicates: Vec<AuditIssue>,
        orphans: Vec<AuditIssue>,
        graph_lines: Vec<String>,
    ) -> Self {
        let mut table_state = TableState::default();
        if !duplicates.is_empty() {
            table_state.select(Some(0));
        }

        Self {
            active_tab: ActiveTab::Duplicates,
            duplicates,
            orphans,
            graph_lines,
            table_state,
            graph_scroll: 0,
            should_quit: false,
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Duplicates => ActiveTab::Orphans,
            ActiveTab::Orphans => ActiveTab::TrafficGraph,
            ActiveTab::TrafficGraph => ActiveTab::Duplicates,
        };
        self.reset_selection();
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Duplicates => ActiveTab::TrafficGraph,
            ActiveTab::Orphans => ActiveTab::Duplicates,
            ActiveTab::TrafficGraph => ActiveTab::Orphans,
        };
        self.reset_selection();
    }

    fn reset_selection(&mut self) {
        let count = match self.active_tab {
            ActiveTab::Duplicates => self.duplicates.len(),
            ActiveTab::Orphans => self.orphans.len(),
            ActiveTab::TrafficGraph => 0,
        };

        if count > 0 {
            self.table_state.select(Some(0));
        } else {
            self.table_state.select(None);
        }
        self.graph_scroll = 0;
    }

    pub fn next_item(&mut self) {
        let count = match self.active_tab {
            ActiveTab::Duplicates => self.duplicates.len(),
            ActiveTab::Orphans => self.orphans.len(),
            ActiveTab::TrafficGraph => {
                if self.graph_scroll < self.graph_lines.len().saturating_sub(1) {
                    self.graph_scroll += 1;
                }
                return;
            }
        };

        if count == 0 {
            return;
        }

        let i = match self.table_state.selected() {
            Some(i) => (i + 1) % count,
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        let count = match self.active_tab {
            ActiveTab::Duplicates => self.duplicates.len(),
            ActiveTab::Orphans => self.orphans.len(),
            ActiveTab::TrafficGraph => {
                self.graph_scroll = self.graph_scroll.saturating_sub(1);
                return;
            }
        };

        if count == 0 {
            return;
        }

        let i = match self.table_state.selected() {
            Some(i) => (i + count - 1) % count,
            None => 0,
        };
        self.table_state.select(Some(i));
    }
}