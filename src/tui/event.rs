use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::time::Duration;
use tokio::sync::mpsc;

pub enum AppEvent {
    Input(KeyEvent),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        tokio::spawn(async move {
            loop {
                // Опрос событий терминала с коротким таймаутом
                if event::poll(Duration::from_millis(20)).unwrap_or(false) {
                    if let Ok(CrosstermEvent::Key(key)) = event::read() {
                        // Игнорируем события Release, обрабатываем только нажатия
                        if key.kind == KeyEventKind::Press {
                            let _ = event_tx.send(AppEvent::Input(key));
                        }
                    }
                }

                tokio::time::sleep(tick_rate).await;
                let _ = event_tx.send(AppEvent::Tick);
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}