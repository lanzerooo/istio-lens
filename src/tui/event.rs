use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::time::Duration;
use tokio::sync::mpsc;

pub enum AppEvent {
    Input(KeyEvent),
    Resize,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if key.kind == KeyEventKind::Press {
                                let _ = event_tx.send(AppEvent::Input(key));
                            }
                        }
                        Ok(CrosstermEvent::Resize(_, _)) => {
                            let _ = event_tx.send(AppEvent::Resize);
                        }
                        _ => {}
                    }
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}