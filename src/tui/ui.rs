use super::app::{ActiveTab, AppState, InputMode};
use crate::analyzer::IssueSeverity;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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

    let crit_count = state
        .duplicates
        .iter()
        .chain(&state.orphans)
        .filter(|i| i.severity == IssueSeverity::Critical)
        .count();

    let warn_count = state.duplicates.len() + state.orphans.len() - crit_count;

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
    if state.traces.is_empty() {
        let empty = Paragraph::new(" Маршруты не найдены для выбранного namespace.")
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Граф трафика "));
        f.render_widget(empty, area);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);

    // 1. Левая колонка: список маршрутов
    let items: Vec<Line> = state
        .traces
        .iter()
        .enumerate()
        .map(|(idx, trace)| {
            let is_sel = idx == state.selected_trace_index;
            let prefix = if is_sel { "▶ " } else { "  " };
            let style = if is_sel {
                Style::default().fg(COLOR_CYAN).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_TEXT)
            };

            Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{}/{}", trace.vs_namespace, trace.vs_name), style),
                Span::styled(format!(" [{}]", trace.uri_match), Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let list = Paragraph::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Маршруты (↑/↓ для выбора) ")
            .border_style(Style::default().fg(COLOR_SURFACE)),
    );
    f.render_widget(list, cols[0]);

    // 2. Правая колонка: сквозной Hop-пайплайн
    let current_trace = &state.traces[state.selected_trace_index];
    let mut canvas = Vec::new();

    // Блок 1: Gateway
    canvas.push(Line::from(vec![
        Span::styled("┌── 🌐 1. GATEWAY ────────────────────────────────────────────────────────┐", Style::default().fg(COLOR_CYAN)),
    ]));
    canvas.push(Line::from(vec![
        Span::styled("│  Name:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(&current_trace.gateway_name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));
    canvas.push(Line::from(vec![
        Span::styled("│  Hosts: ", Style::default().fg(Color::DarkGray)),
        Span::styled(current_trace.gateway_hosts.join(", "), Style::default().fg(COLOR_CYAN)),
    ]));
    canvas.push(Line::from(vec![
        Span::styled("└───┬────────────────────────────────────────────────────────────────────┘", Style::default().fg(COLOR_CYAN)),
    ]));

    canvas.push(Line::from(Span::styled("    │  (Ingress Routing Rule)", Style::default().fg(Color::DarkGray))));
    canvas.push(Line::from(Span::styled("    ▼", Style::default().fg(COLOR_BLUE))));

    // Блок 2: VirtualService
    canvas.push(Line::from(vec![
        Span::styled("┌── 🔀 2. VIRTUAL SERVICE ───────────────────────────────────────────────┐", Style::default().fg(COLOR_BLUE)),
    ]));
    canvas.push(Line::from(vec![
        Span::styled("│  Name:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}/{}", current_trace.vs_namespace, current_trace.vs_name), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));
    canvas.push(Line::from(vec![
        Span::styled("│  Match: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&current_trace.uri_match, Style::default().fg(COLOR_WARN).add_modifier(Modifier::BOLD)),
        Span::styled(" ──► Target: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&current_trace.service_host, Style::default().fg(COLOR_OK)),
    ]));
    canvas.push(Line::from(vec![
        Span::styled("└───┬────────────────────────────────────────────────────────────────────┘", Style::default().fg(COLOR_BLUE)),
    ]));

    canvas.push(Line::from(Span::styled("    │  (Forward to K8s Service)", Style::default().fg(Color::DarkGray))));
    canvas.push(Line::from(Span::styled("    ▼", Style::default().fg(COLOR_OK))));

    // Блок 3: K8s Service & Destinations
    for target in &current_trace.targets {
        let (port_str, selector_str) = match &target.service_meta {
            Some(meta) => {
                let p = if meta.ports.is_empty() { "default".into() } else { meta.ports.join(", ") };
                let s = if meta.selector.is_empty() {
                    "No selector".into()
                } else {
                    meta.selector.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(", ")
                };
                (p, s)
            }
            None => ("Unknown".into(), "External / No Service".into()),
        };

        canvas.push(Line::from(vec![
            Span::styled("┌── ⚙️  3. K8S SERVICE & ROUTE TARGET ─────────────────────────────────────┐", Style::default().fg(COLOR_OK)),
        ]));
        canvas.push(Line::from(vec![
            Span::styled("│  Host:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(&current_trace.service_host, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" | Ports: ", Style::default().fg(Color::DarkGray)),
            Span::styled(port_str, Style::default().fg(COLOR_CYAN)),
        ]));
        canvas.push(Line::from(vec![
            Span::styled("│  Selector: ", Style::default().fg(Color::DarkGray)),
            Span::styled(selector_str, Style::default().fg(COLOR_TEXT)),
        ]));
        canvas.push(Line::from(vec![
            Span::styled("│  Policy:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("DestinationRule: {} | Subset: '{}' | Weight: {}%", 
                target.destination_rule.as_deref().unwrap_or("None"),
                target.subset_name,
                target.weight
            ), Style::default().fg(COLOR_WARN)),
        ]));
        canvas.push(Line::from(vec![
            Span::styled("└───┬────────────────────────────────────────────────────────────────────┘", Style::default().fg(COLOR_OK)),
        ]));

        canvas.push(Line::from(Span::styled("    │  (Endpoint Label Selection)", Style::default().fg(Color::DarkGray))));
        canvas.push(Line::from(Span::styled("    ▼", Style::default().fg(Color::Magenta))));

        // Блок 4: Pods
        let ready_count = target.matching_pods.iter().filter(|p| p.is_ready).count();
        let total_count = target.matching_pods.len();

        let (pod_col, pod_status_text) = if total_count == 0 {
            (COLOR_CRIT, "✖ 0 подов найдено (Dead End!)")
        } else if ready_count < total_count {
            (COLOR_WARN, "▲ Частичная деградация подов")
        } else {
            (COLOR_OK, "● Все реплики здоровы")
        };

        canvas.push(Line::from(vec![
            Span::styled("┌── 📦 4. APPLICATION PODS & WORKLOADS ───────────────────────────────────┐", Style::default().fg(pod_col)),
        ]));
        canvas.push(Line::from(vec![
            Span::styled("│  Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} (Ready: {}/{})", pod_status_text, ready_count, total_count), Style::default().fg(pod_col).add_modifier(Modifier::BOLD)),
        ]));

        if target.matching_pods.is_empty() {
            canvas.push(Line::from(vec![
                Span::styled("│  ✖ ВНИМАНИЕ: Нет подов, удовлетворяющих селекторам сервиса и сабсета!", Style::default().fg(COLOR_CRIT)),
            ]));
        } else {
            for pod in target.matching_pods.iter().take(4) {
                let badge = if pod.is_ready { "[Ready 1/1]" } else { "[Not Ready]" };
                let b_style = if pod.is_ready { Style::default().fg(COLOR_OK) } else { Style::default().fg(COLOR_CRIT) };

                canvas.push(Line::from(vec![
                    Span::styled("│  • Pod: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:<30}", pod.name), Style::default().fg(Color::White)),
                    Span::styled(format!(" Phase: {:<9} ", pod.phase), Style::default().fg(COLOR_TEXT)),
                    Span::styled(badge, b_style),
                ]));
            }
            if target.matching_pods.len() > 4 {
                canvas.push(Line::from(vec![
                    Span::styled(format!("│  ... и еще {} подов скрыто", target.matching_pods.len() - 4), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        canvas.push(Line::from(vec![
            Span::styled("└────────────────────────────────────────────────────────────────────────┘", Style::default().fg(pod_col)),
        ]));
    }

    let detail = Paragraph::new(canvas).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Сквозной маршрут трафика: Gateway -> Pod ")
            .border_style(Style::default().fg(COLOR_SURFACE)),
    );
    f.render_widget(detail, cols[1]);
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