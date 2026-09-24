mod analyzer;
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
use graph::TrafficGraph;
use k8s::K8sDiscovery;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, panic, time::Duration};
use tui::{
    app::{ActiveTab, AppState},
    event::{AppEvent, EventHandler},
    ui,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Сбор данных из кластера Kubernetes
    println!("Инициализация подключения к Kubernetes...");
    let discovery = K8sDiscovery::new().await?;

    println!("Получение конфигураций Istio и Service ресурсов...");
    let snapshot = discovery.fetch_snapshot().await?;

    // 2. Статический анализ
    let analyzer = Analyzer::new(&snapshot);
    let (duplicates, orphans) = analyzer.run_all();

    // 3. Построение топологии
    let topology = TrafficGraph::build(&snapshot);
    let graph_display = topology.to_display_lines();

    // 4. Инициализация TUI терминала
    setup_panic_hook();
    let mut terminal = setup_terminal()?;
    let mut events = EventHandler::new(Duration::from_millis(250));
    let mut state = AppState::new(duplicates, orphans, graph_display);

    // 5. Главный цикл событий
    while !state.should_quit {
        terminal.draw(|f| ui::render(f, &mut state))?;

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Input(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
                    KeyCode::Tab => state.next_tab(),
                    KeyCode::BackTab => state.prev_tab(),
                    KeyCode::Char('1') => state.active_tab = ActiveTab::Duplicates,
                    KeyCode::Char('2') => state.active_tab = ActiveTab::Orphans,
                    KeyCode::Char('3') => state.active_tab = ActiveTab::TrafficGraph,
                    KeyCode::Down | KeyCode::Char('j') => state.next_item(),
                    KeyCode::Up | KeyCode::Char('k') => state.previous_item(),
                    _ => {}
                },
                AppEvent::Tick => {}
            }
        }
    }

    // 6. Корректное завершение
    restore_terminal(&mut terminal)?;
    Ok(())
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;
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
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, cursor::Show);
        original_hook(panic_info);
    }));
}