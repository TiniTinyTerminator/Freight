mod app;
pub mod client;
mod config;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use app::{App, DataEvent};
use client::Client;

/// Launch the registry admin TUI (blocks the calling thread).
///
/// `url`   — registry base URL (e.g. `http://localhost:7878`)
/// `token` — optional pre-loaded API token; shows the login screen if absent
pub fn run(url: String, token: Option<String>) -> Result<()> {
    // Prefer CLI/env token; fall back to persisted config file.
    let (url, token) = match (token, config::TuiConfig::load()) {
        (Some(tok), _)        => (url, Some(tok)),
        (None, Some(cfg))     => (cfg.url, Some(cfg.token)),
        (None, None)          => (url, None),
    };

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_async(url, token))
}

async fn run_async(url: String, token: Option<String>) -> Result<()> {
    let client  = Client::new(url.clone(), token);
    let mut app = App::new(client, url);

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let result = event_loop(&mut term, &mut app).await;

    // Restore terminal unconditionally
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    term.show_cursor()?;

    result
}

async fn event_loop(
    term: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app:  &mut App,
) -> Result<()> {
    let (data_tx, mut data_rx) = mpsc::channel::<DataEvent>(64);
    let (ev_tx,   mut ev_rx)   = mpsc::channel::<Event>(64);

    // Blocking event reader — forwards Key and Mouse events; ignores resize/focus.
    let ev_tx2 = ev_tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(ev) = event::read() {
                    match ev {
                        Event::Key(_) | Event::Mouse(_) => {
                            if ev_tx2.blocking_send(ev).is_err() { break; }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    // Initial data load.
    app.load_me(data_tx.clone());
    app.load_current_tab(data_tx.clone());

    loop {
        term.draw(|f| ui::draw(f, app))?;

        tokio::select! {
            ev = ev_rx.recv() => {
                match ev {
                    Some(Event::Key(k)) => {
                        if app.handle_key(k, &data_tx) { break; }
                    }
                    Some(Event::Mouse(m)) => {
                        app.handle_mouse(m, &data_tx);
                    }
                    _ => {}
                }
            }
            data = data_rx.recv() => {
                if let Some(d) = data { app.handle_data(d, &data_tx); }
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                // periodic redraw tick — updates spinner and relative timestamps
            }
        }
    }

    Ok(())
}
