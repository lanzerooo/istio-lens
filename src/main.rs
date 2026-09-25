mod analyzer;
mod diagnostics;
mod error;
mod graph;
mod k8s;
mod model;
mod tui;

use analyzer::Analyzer;
use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use diagnostics::DiagnosticsCollector;
use graph::TrafficGraph;
use k8s::K8sDiscovery;
use kube::Client;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, panic};
use tui::{
    app::{ActiveTab, AppState, InputMode},
    event::{AppEvent, EventHandler},
    ui,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Инициализация подключения к Kubernetes
    println!("Подключение к Kubernetes через kubeconfig...");
    let client = Client::try_default().await?;
    let discovery = K8sDiscovery::new().await?;

    println!("Сбор сетевых ресурсов (Gateways, VirtualServices, DestinationRules, Services, Pods)...");
    let snapshot = discovery.fetch_snapshot().await?;

    // 2. Сбор диагностических данных (Causal Incident Timeline, FinOps Tetris, Admission Webhooks)
    println!("Сбор диагностических данных (Tetris, Webhooks, Timelines)...");
    let diag_collector = DiagnosticsCollector::new(client);
    let diag_snapshot = diag_collector.collect().await?;

    // 3. Статический аудит дубликатов и сирот
    let analyzer = Analyzer::new(&snapshot);
    let (duplicates, orphans) = analyzer.run_all();

    // 4. Построение сквозных цепочек трафика (Gateway -> Pods)
    let traces = TrafficGraph::build_traces(&snapshot, None);

    // 5. Инициализация терминала и перехват паники для безопасного сброса
    setup_panic_hook();
    let mut terminal = setup_terminal()?;
    let mut events = EventHandler::new();
    let mut state = AppState::new(
        duplicates,
        orphans,
        traces,
        snapshot.namespaces.clone(),
        diag_snapshot.incidents,
        diag_snapshot.node_profiles,
        diag_snapshot.webhook_reports,
    );

    // Флаг реактивной перерисовки (0% CPU в простое)
    let mut dirty = true;

    // 6. Главный цикл событий
    while !state.should_quit {
        if dirty {
            terminal.draw(|f| ui::render(f, &mut state))?;
            dirty = false;
        }

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Resize => {
                    dirty = true;
                }
                AppEvent::Input(key) => {
                    dirty = true;

                    // 6.1. Режим строкового поиска
                    if state.input_mode == InputMode::Searching {
                        match key.code {
                            KeyCode::Esc => {
                                state.search_query.clear();
                                state.input_mode = InputMode::Normal;
                                state.reset_selection();
                            }
                            KeyCode::Enter => {
                                state.input_mode = InputMode::Normal;
                                state.reset_selection();
                            }
                            KeyCode::Backspace => {
                                state.search_query.pop();
                                state.reset_selection();
                            }
                            KeyCode::Char(c) => {
                                state.search_query.push(c);
                                state.reset_selection();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // 6.2. Модальный селектор Namespace Scope
                    if state.input_mode == InputMode::NamespaceSelect {
                        match key.code {
                            KeyCode::Esc => {
                                state.input_mode = InputMode::Normal;
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.next_item();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                state.previous_item();
                            }
                            KeyCode::Enter => {
                                state.selected_namespace = if state.ns_selector_index == 0 {
                                    None
                                } else {
                                    state.namespaces.get(state.ns_selector_index - 1).cloned()
                                };

                                // Перестраиваем граф трафика под выбранный Scope
                                state.traces = TrafficGraph::build_traces(
                                    &snapshot,
                                    state.selected_namespace.as_deref(),
                                );
                                state.input_mode = InputMode::Normal;
                                state.reset_selection();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // 6.3. Модальное окно просмотра манифеста
                    if state.show_details {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                                state.show_details = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // 6.4. Базовая навигация
                    match key.code {
                        KeyCode::Char('q') => {
                            state.should_quit = true;
                        }
                        KeyCode::Char('/') => {
                            state.input_mode = InputMode::Searching;
                        }
                        KeyCode::Char('n') => {
                            state.input_mode = InputMode::NamespaceSelect;
                        }
                        KeyCode::Enter => {
                            if matches!(state.active_tab, ActiveTab::Duplicates | ActiveTab::Orphans) {
                                state.show_details = true;
                            }
                        }
                        KeyCode::Tab => {
                            state.next_tab();
                        }
                        KeyCode::BackTab => {
                            state.prev_tab();
                        }
                        KeyCode::Char('1') => {
                            state.active_tab = ActiveTab::Duplicates;
                            state.reset_selection();
                        }
                        KeyCode::Char('2') => {
                            state.active_tab = ActiveTab::Orphans;
                            state.reset_selection();
                        }
                        KeyCode::Char('3') => {
                            state.active_tab = ActiveTab::TrafficGraph;
                            state.reset_selection();
                        }
                        KeyCode::Char('4') => {
                            state.active_tab = ActiveTab::IncidentTimeline;
                            state.reset_selection();
                        }
                        KeyCode::Char('5') => {
                            state.active_tab = ActiveTab::NodeTetris;
                            state.reset_selection();
                        }
                        KeyCode::Char('6') => {
                            state.active_tab = ActiveTab::WebhookAuditor;
                            state.reset_selection();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.next_item();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            state.previous_item();
                        }
                        KeyCode::Char('d') => {
                            if state.active_tab == ActiveTab::TrafficGraph {
                                state.scroll_detail_down();
                            }
                        }
                        KeyCode::Char('u') => {
                            if state.active_tab == ActiveTab::TrafficGraph {
                                state.scroll_detail_up();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        cursor::Hide
    )?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        cursor::Show
    )?;
    Ok(())
}

fn setup_panic_hook() {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            cursor::Show
        );
        original_hook(panic_info);
    }));
}