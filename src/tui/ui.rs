use super::app::{ActiveTab, AppState};
use crate::analyzer::{AuditIssue, IssueSeverity};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

pub fn render(f: &mut Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Верхний бар вкладок
            Constraint::Min(10),   // Основное рабочее пространство
            Constraint::Length(1), // Нижняя панель подсказок
        ])
        .split(f.size());

    render_tabs(f, state, chunks[0]);

    match state.active_tab {
        ActiveTab::Duplicates => render_issues(f, state, chunks[1], true),
        ActiveTab::Orphans => render_issues(f, state, chunks[1], false),
        ActiveTab::TrafficGraph => render_graph(f, state, chunks[1]),
    }

    render_footer(f, chunks[2]);
}

fn render_tabs(f: &mut Frame, state: &AppState, area: Rect) {
    let titles = vec!["[1] Дубликаты", "[2] Неиспользуемые", "[3] Граф трафика"];
    let idx = match state.active_tab {
        ActiveTab::Duplicates => 0,
        ActiveTab::Orphans => 1,
        ActiveTab::TrafficGraph => 2,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Istio Lens - Service Mesh Inspector "),
        )
        .select(idx)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn render_issues(f: &mut Frame, state: &mut AppState, area: Rect, is_duplicates: bool) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let issues: &[AuditIssue] = if is_duplicates {
        &state.duplicates
    } else {
        &state.orphans
    };

    let selected_index = state.table_state.selected();

    let rows: Vec<Row> = issues
        .iter()
        .map(|item| {
            let (severity_str, style) = match item.severity {
                IssueSeverity::Critical => ("CRIT", Style::default().fg(Color::Red)),
                IssueSeverity::Warning => ("WARN", Style::default().fg(Color::Yellow)),
            };

            Row::new(vec![
                Cell::from(severity_str).style(style),
                Cell::from(item.kind.clone()),
                Cell::from(item.namespace.clone()),
                Cell::from(item.resource.clone()),
                Cell::from(item.description.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(18),
            Constraint::Length(15),
            Constraint::Length(25),
            Constraint::Min(30),
        ],
    )
    .header(
        Row::new(vec![
            "Уровень",
            "Тип",
            "Namespace",
            "Имя ресурса",
            "Описание проблемы",
        ])
        .style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::White),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Обнаруженные дефекты конфигурации "),
    )
    .highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 52))
            .add_modifier(Modifier::BOLD),
    );

    f.render_stateful_widget(table, main_chunks[0], &mut state.table_state);

    let detail_text = if let Some(idx) = selected_index {
        if let Some(issue) = issues.get(idx) {
            format!(
                "Ресурс: {}/{}\nТип: {}\nВажность: {:?}\n\nОписание проблемы:\n{}",
                issue.namespace, issue.resource, issue.kind, issue.severity, issue.description
            )
        } else {
            "Нет данных".into()
        }
    } else {
        "Список пуст или элемент не выбран".into()
    };

    let details = Paragraph::new(detail_text)
        .block(Block::default().borders(Borders::ALL).title(" Детализация "))
        .style(Style::default().fg(Color::Gray));

    f.render_widget(details, main_chunks[1]);
}

fn render_graph(f: &mut Frame, state: &AppState, area: Rect) {
    let visible_lines: Vec<Line> = state
        .graph_lines
        .iter()
        .skip(state.graph_scroll)
        .map(|l| Line::from(Span::styled(l, Style::default().fg(Color::Green))))
        .collect();

    let paragraph = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Маршрутизация: Gateway -> VirtualService -> Service -> DestinationRule "),
    );

    f.render_widget(paragraph, area);
}

fn render_footer(f: &mut Frame, area: Rect) {
    let help = Span::raw(" [q] Выход | [Tab / 1-3] Вкладки | [↑/↓, j/k] Навигация ");
    f.render_widget(
        Paragraph::new(help).style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        area,
    );
}