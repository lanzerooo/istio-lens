use super::app::{ActiveTab, AppState, InputMode};
use crate::analyzer::IssueSeverity;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Frame,
};

// Цветовая палитра Catppuccin Mocha
const COLOR_CRIT: Color = Color::Rgb(243, 139, 168);  // Red
const COLOR_WARN: Color = Color::Rgb(250, 179, 135);  // Peach
const COLOR_OK: Color = Color::Rgb(166, 227, 161);    // Green
const COLOR_CYAN: Color = Color::Rgb(137, 220, 235);  // Sky
const COLOR_BLUE: Color = Color::Rgb(137, 180, 250);  // Blue
const COLOR_TEXT: Color = Color::Rgb(205, 214, 244);  // Foreground
const COLOR_SURFACE: Color = Color::Rgb(49, 50, 68);  // Surface

pub fn render(f: &mut Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Карточки метрик (Summary)
            Constraint::Length(3), // Вкладки
            Constraint::Min(10),   // Основное тело
            Constraint::Length(1), // Хелп-бар или поле ввода
        ])
        .split(f.size());

    render_summary_cards(f, state, chunks[0]);
    render_tabs(f, state, chunks[1]);

    match state.active_tab {
        ActiveTab::Duplicates => render_issues_table(f, state, chunks[2], true),
        ActiveTab::Orphans => render_issues_table(f, state, chunks[2], false),
        ActiveTab::TrafficGraph => render_graph_view(f, state, chunks[2]),
    }

    render_footer(f, state, chunks[3]);

    // Рендеринг оверлеев поверх интерфейса
    if state.input_mode == InputMode::NamespaceSelect {
        render_namespace_modal(f, state);
    } else if state.show_details {
        render_details_modal(f, state);
    }
}

fn render_summary_cards(f: &mut Frame, state: &AppState, area: Rect) {
    let cards = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let crit_count = state.duplicates.iter().chain(&state.orphans)
        .filter(|i| i.severity == IssueSeverity::Critical)
        .count();

    let warn_count = state.duplicates.len() + state.orphans.len() - crit_count;

    // Card 1: Mesh Health
    let (health_text, health_col) = if crit_count > 0 {
        ("CRITICAL", COLOR_CRIT)
    } else if warn_count > 0 {
        ("WARNING", COLOR_WARN)
    } else {
        ("HEALTHY", COLOR_OK)
    };

    let p1 = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ● ", Style::default().fg(health_col)),
            Span::styled(health_text, Style::default().fg(health_col).add_modifier(Modifier::BOLD)),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Mesh Health ")
            .border_style(Style::default().fg(COLOR_SURFACE)),
    );
    f.render_widget(p1, cards[0]);

    // Card 2: Critical issues
    let p2 = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} критических", crit_count),
            Style::default().fg(COLOR_CRIT).add_modifier(Modifier::BOLD),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Critical ")
            .border_style(Style::default().fg(COLOR_SURFACE)),
    );
    f.render_widget(p2, cards[1]);

    // Card 3: Warnings
    let p3 = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} предупреждений", warn_count),
            Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Warnings ")
            .border_style(Style::default().fg(COLOR_SURFACE)),
    );
    f.render_widget(p3, cards[2]);

    // Card 4: Namespace Filter
    let ns_label = state.selected_namespace.as_deref().unwrap_or("All Namespaces");
    let p4 = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  🎯 {}", ns_label),
            Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Active Scope [n] ")
            .border_style(Style::default().fg(COLOR_SURFACE)),
    );
    f.render_widget(p4, cards[3]);
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
                .border_type(BorderType::Rounded)
                .title(" Istio Lens "),
        )
        .select(idx)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn render_issues_table(f: &mut Frame, state: &mut AppState, area: Rect, is_duplicates: bool) {
    let issues = state.filtered_issues(is_duplicates);

    let rows: Vec<Row> = issues
        .iter()
        .map(|item| {
            let (label, style) = match item.severity {
                IssueSeverity::Critical => ("CRIT", Style::default().fg(COLOR_CRIT).add_modifier(Modifier::BOLD)),
                IssueSeverity::Warning => ("WARN", Style::default().fg(COLOR_WARN)),
            };

            Row::new(vec![
                Cell::from(label).style(style),
                Cell::from(item.kind.clone()),
                Cell::from(item.namespace.clone()).style(Style::default().fg(COLOR_CYAN)),
                Cell::from(item.resource.clone()).style(Style::default().fg(COLOR_TEXT)),
                Cell::from(item.description.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(25),
            Constraint::Min(30),
        ],
    )
    .header(
        Row::new(vec!["Уровень", "Тип", "Namespace", "Имя ресурса", "Описание"])
            .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::White)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Обнаруженные аномалии [Enter: детали, /: поиск] "),
    )
    .highlight_style(Style::default().bg(COLOR_SURFACE).add_modifier(Modifier::BOLD));

    f.render_stateful_widget(table, area, &mut state.table_state);
}

fn render_graph_view(f: &mut Frame, state: &AppState, area: Rect) {
    let visible_lines: Vec<Line> = state
        .graph_lines
        .iter()
        .skip(state.graph_scroll)
        .map(|l| {
            let col = if l.contains("Gateway") {
                COLOR_CYAN
            } else if l.contains("VirtualService") {
                COLOR_BLUE
            } else if l.contains("0 endpoints") {
                COLOR_CRIT
            } else if l.contains("ServiceEntry") {
                COLOR_WARN
            } else {
                COLOR_OK
            };
            Line::from(Span::styled(l, Style::default().fg(col)))
        })
        .collect();

    let paragraph = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Топология трафика: Ingress Gateway -> VirtualService -> Service -> Subsets "),
    );
    f.render_widget(paragraph, area);
}

fn render_footer(f: &mut Frame, state: &AppState, area: Rect) {
    let content = match state.input_mode {
        InputMode::Searching => format!(" ПОИСК: {}█ (Esc: отмена, Enter: применить)", state.search_query),
        _ => " [q] Выход | [/] Поиск | [n] Scope | [Enter] Детали | [Tab/1-3] Вкладки | [j/k] Навигация ".into(),
    };

    let style = if state.input_mode == InputMode::Searching {
        Style::default().bg(COLOR_BLUE).fg(Color::Black).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(COLOR_SURFACE).fg(COLOR_TEXT)
    };

    f.render_widget(Paragraph::new(content).style(style), area);
}

/// Модальное окно выбора Namespace
fn render_namespace_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(50, 60, f.size());
    f.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(Span::styled(
            if state.ns_selector_index == 0 { " > [0] All Namespaces < " } else { "   [0] All Namespaces   " },
            if state.ns_selector_index == 0 { Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD) } else { Style::default() },
        ))
    ];

    for (idx, ns) in state.namespaces.iter().enumerate() {
        let is_selected = state.ns_selector_index == idx + 1;
        let prefix = if is_selected { " > " } else { "   " };
        let style = if is_selected {
            Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(format!("{}[{}] {}", prefix, idx + 1, ns), style)));
    }

    let block = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Выберите Namespace [↑/↓, Enter: выбрать, Esc: отмена] ")
            .style(Style::default().bg(Color::Black)),
    );
    f.render_widget(block, area);
}

/// Модальное окно просмотра деталей и сырого YAML
fn render_details_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(70, 70, f.size());
    f.render_widget(Clear, area);

    let is_dup = state.active_tab == ActiveTab::Duplicates;
    let issues = state.filtered_issues(is_dup);

    let content = if let Some(idx) = state.table_state.selected() {
        if let Some(issue) = issues.get(idx) {
            let yaml_repr = serde_yaml::to_string(&issue).unwrap_or_else(|_| "Ошибка сериализации".into());
            format!(
                "Детали дефекта конфигурации:\n\n\
                Тип ресурса: {}\n\
                Имя: {}\n\
                Namespace: {}\n\
                Важность: {:?}\n\n\
                Аналитический отчет:\n{}\n\n\
                ---\n\
                Структура дефекта (YAML):\n{}",
                issue.kind, issue.resource, issue.namespace, issue.severity, issue.description, yaml_repr
            )
        } else {
            "Элемент не выбран".into()
        }
    } else {
        "Элемент не выбран".into()
    };

    let block = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Инспектор манифеста [Esc / Enter: закрыть] ")
            .style(Style::default().bg(Color::Black).fg(COLOR_TEXT)),
    );
    f.render_widget(block, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}