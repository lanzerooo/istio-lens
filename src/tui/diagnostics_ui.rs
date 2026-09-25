use crate::diagnostics::{tetris::NodeTetrisProfile, timeline::CausalAnalysis, webhooks::{WebhookAuditReport, WebhookRiskLevel}};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

// 1. Рендеринг Tetris & OOM Hazard Map
pub fn render_tetris_view(f: &mut Frame, profiles: &[NodeTetrisProfile], area: Rect) {
    let mut rows = Vec::new();

    for p in profiles {
        let mem_req_pct = p.memory.requests_pct();
        let mem_lim_pct = p.memory.limits_pct();
        let hazard = p.memory.hazard_ratio();

        // Графический индикатор: Allocatable | Requests | Overcommit Hazard
        let mem_bar = render_tetris_bar(mem_req_pct, mem_lim_pct);

        let (hazard_style, hazard_badge) = if hazard > 2.0 {
            (Style::default().fg(Color::Red).add_modifier(Modifier::BOLD), format!("{:.1}x (КРИТИЧЕСКИЙ)", hazard))
        } else if hazard > 1.2 {
            (Style::default().fg(Color::Yellow), format!("{:.1}x (Повышенный)", hazard))
        } else {
            (Style::default().fg(Color::Green), format!("{:.1}x (Безопасно)", hazard))
        };

        rows.push(Row::new(vec![
            Cell::from(p.node_name.clone()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Cell::from(format!("{} MiB", p.memory.allocatable)),
            Cell::from(mem_bar),
            Cell::from(hazard_badge).style(hazard_style),
            Cell::from(format!("BestEffort: {}, Burstable: {}", p.best_effort_pods.len(), p.burstable_pods.len())),
        ]));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Length(12),
            Constraint::Length(28),
            Constraint::Length(20),
            Constraint::Min(25),
        ],
    )
    .header(
        Row::new(vec!["Node", "Память", "Tetris: [Requests | Limits Overcommit]", "OOM Hazard Ratio", "Жертвы OOM"])
            .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" FinOps Tetris: Срезы памяти нод и риск каскадного OOM "),
    );

    f.render_widget(table, area);
}

fn render_tetris_bar(req_pct: f64, lim_pct: f64) -> String {
    let total_slots = 18;
    let req_slots = ((req_pct / 100.0) * total_slots as f64).round() as usize;
    let lim_slots = ((lim_pct / 100.0) * total_slots as f64).round() as usize;

    let mut bar = String::with_capacity(total_slots + 4);
    bar.push('[');

    for i in 0..total_slots {
        if i < req_slots {
            bar.push('█'); // Базовый резерв по Requests
        } else if i < lim_slots {
            bar.push('▓'); // Опасная зона Limits (Overcommit)
        } else {
            bar.push('░'); // Свободное место
        }
    }
    bar.push(']');

    if lim_pct > 100.0 {
        bar.push_str(" ⚠️ >100%");
    }
    bar
}

// 2. Рендеринг Causal Incident Timeline
pub fn render_incident_view(f: &mut Frame, incident: &CausalAnalysis, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(10)])
        .split(area);

    // Вердикт первопричины
    let banner_col = if incident.is_node_level_failure { Color::Red } else { Color::Yellow };
    let verdict_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" АВТО-ВЕРДИКТ: ", Style::default().bg(banner_col).fg(Color::Black).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}", incident.root_cause_verdict), Style::default().fg(banner_col).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled(
            format!(" Pod: {}/{} | Exit Code: {:?}", incident.namespace, incident.pod_name, incident.exit_code),
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Causal Root Cause Analyzer "));
    f.render_widget(verdict_p, chunks[0]);

    // Хронологическая лента
    let events: Vec<Line> = incident
        .events
        .iter()
        .map(|e| {
            let (icon, col) = match e.severity {
                crate::diagnostics::timeline::IncidentSeverity::Fatal => ("✖ [CRIT]", Color::Red),
                crate::diagnostics::timeline::IncidentSeverity::Warning => ("▲ [WARN]", Color::Yellow),
                crate::diagnostics::timeline::IncidentSeverity::Info => ("● [INFO]", Color::Cyan),
            };

            Line::from(vec![
                Span::styled(format!(" {} ", e.timestamp.format("%H:%M:%S%.3f")), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<8} ", icon), Style::default().fg(col).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<16} ", format!("[{}]", e.source)), Style::default().fg(Color::White)),
                Span::styled(format!("{:<20} ", e.reason), Style::default().fg(col)),
                Span::styled(e.message.clone(), Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    let timeline_p = Paragraph::new(events).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Хронологическая лента событий инцидента (Time-Travel Scrubber) "),
    );
    f.render_widget(timeline_p, chunks[1]);
}

// 3. Рендеринг Webhook Trap Detector
pub fn render_webhooks_view(f: &mut Frame, webhooks: &[WebhookAuditReport], area: Rect) {
    let rows: Vec<Row> = webhooks
        .iter()
        .map(|w| {
            let (risk_badge, style) = match w.risk_level {
                WebhookRiskLevel::ClusterBlocker => ("BLOCKER", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)),
                WebhookRiskLevel::Warning => ("WARNING", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                WebhookRiskLevel::Safe => ("HEALTHY", Style::default().fg(Color::Green)),
            };

            Row::new(vec![
                Cell::from(risk_badge).style(style),
                Cell::from(w.hook_type),
                Cell::from(w.name.clone()),
                Cell::from(w.service_ref.clone()),
                Cell::from(w.failure_policy.clone()),
                Cell::from(format!("{}s", w.timeout_seconds)),
                Cell::from(w.risk_explanation.clone()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(25),
            Constraint::Length(25),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(30),
        ],
    )
    .header(
        Row::new(vec!["Статус", "Тип", "Имя вебхука", "Целевой сервис", "Policy", "Таймаут", "Анализ риска"])
            .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Admission Webhook Trap Inspector: Анализ скрытых точек отказа API "),
    );

    f.render_widget(table, area);
}