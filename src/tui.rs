//! Terminal UI for live request monitoring
//!
//! Provides a ratatui-based dashboard showing live request/response logs,
//! server stats, and function overview.

#[cfg(feature = "tui")]
pub mod ui {
    use crossterm::{
        event::{self, Event, KeyCode, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{
        backend::CrosstermBackend,
        layout::{Constraint, Direction, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
        Frame, Terminal,
    };
    use std::io;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::broadcast;

    /// A request event sent from the HTTP handler to the TUI
    #[derive(Clone, Debug)]
    pub struct RequestEvent {
        pub timestamp: String,
        pub method: String,
        pub path: String,
        pub status: u16,
        pub duration_ms: u64,
        pub function: String,
        pub response_bytes: usize,
    }

    /// TUI application state
    struct App {
        requests: Vec<RequestEvent>,
        table_state: TableState,
        scroll_locked: bool, // auto-scroll to bottom
        total_requests: u64,
        total_errors: u64,
        start_time: Instant,
        server_info: ServerInfo,
    }

    #[derive(Clone)]
    pub struct ServerInfo {
        pub version: String,
        pub ports: Vec<(String, u16)>, // (gateway_name, port)
        pub functions: Vec<String>,
        pub watching: bool,
    }

    impl App {
        fn new(server_info: ServerInfo) -> Self {
            let mut table_state = TableState::default();
            table_state.select(None);
            Self {
                requests: Vec::with_capacity(1000),
                table_state,
                scroll_locked: true,
                total_requests: 0,
                total_errors: 0,
                start_time: Instant::now(),
                server_info,
            }
        }

        fn push_event(&mut self, event: RequestEvent) {
            self.total_requests += 1;
            if event.status >= 400 {
                self.total_errors += 1;
            }
            self.requests.push(event);
            // Keep max 500 entries to avoid memory bloat
            if self.requests.len() > 500 {
                self.requests.drain(..100);
            }
            if self.scroll_locked {
                let len = self.requests.len();
                if len > 0 {
                    self.table_state.select(Some(len - 1));
                }
            }
        }

        fn scroll_up(&mut self) {
            self.scroll_locked = false;
            let i = match self.table_state.selected() {
                Some(i) if i > 0 => i - 1,
                Some(i) => i,
                None if !self.requests.is_empty() => self.requests.len() - 1,
                None => 0,
            };
            self.table_state.select(Some(i));
        }

        fn scroll_down(&mut self) {
            let len = self.requests.len();
            if len == 0 {
                return;
            }
            let i = match self.table_state.selected() {
                Some(i) if i + 1 < len => i + 1,
                Some(i) => i,
                None => 0,
            };
            self.table_state.select(Some(i));
            if i == len - 1 {
                self.scroll_locked = true;
            }
        }

        fn scroll_to_bottom(&mut self) {
            self.scroll_locked = true;
            if !self.requests.is_empty() {
                self.table_state.select(Some(self.requests.len() - 1));
            }
        }
    }

    /// Run the TUI event loop. Blocks until user quits (q/Ctrl+C).
    pub async fn run_tui(
        mut rx: broadcast::Receiver<RequestEvent>,
        server_info: ServerInfo,
        shutdown: Arc<tokio::sync::Notify>,
    ) -> anyhow::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut app = App::new(server_info);
        let tick_rate = Duration::from_millis(100);

        loop {
            // Draw
            terminal.draw(|f| draw_ui(f, &mut app))?;

            // Handle events with timeout
            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            shutdown.notify_one();
                            break;
                        }
                        (KeyCode::Up | KeyCode::Char('k'), _) => app.scroll_up(),
                        (KeyCode::Down | KeyCode::Char('j'), _) => app.scroll_down(),
                        (KeyCode::Char('G'), _) | (KeyCode::End, _) => app.scroll_to_bottom(),
                        (KeyCode::Home | KeyCode::Char('g'), _) => {
                            app.scroll_locked = false;
                            app.table_state.select(Some(0));
                        }
                        (KeyCode::Char('c'), _) => {
                            app.requests.clear();
                            app.table_state.select(None);
                        }
                        _ => {}
                    }
                }
            }

            // Drain all available events from channel
            loop {
                match rx.try_recv() {
                    Ok(event) => app.push_event(event),
                    Err(broadcast::error::TryRecvError::Empty) => break,
                    Err(broadcast::error::TryRecvError::Closed) => {
                        // Server shut down
                        break;
                    }
                    Err(broadcast::error::TryRecvError::Lagged(n)) => {
                        tracing::debug!("TUI lagged by {} events", n);
                        break;
                    }
                }
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        Ok(())
    }

    fn draw_ui(f: &mut Frame, app: &mut App) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(5),    // Request log
                Constraint::Length(1), // Status bar
            ])
            .split(f.area());

        draw_header(f, app, chunks[0]);
        draw_request_table(f, app, chunks[1]);
        draw_status_bar(f, app, chunks[2]);
    }

    fn draw_header(f: &mut Frame, app: &App, area: Rect) {
        let uptime = app.start_time.elapsed();
        let uptime_str = if uptime.as_secs() >= 3600 {
            format!(
                "{}h{}m",
                uptime.as_secs() / 3600,
                (uptime.as_secs() % 3600) / 60
            )
        } else if uptime.as_secs() >= 60 {
            format!("{}m{}s", uptime.as_secs() / 60, uptime.as_secs() % 60)
        } else {
            format!("{}s", uptime.as_secs())
        };

        let ports_str: Vec<String> = app
            .server_info
            .ports
            .iter()
            .map(|(name, port)| {
                if name.is_empty() {
                    format!(":{}", port)
                } else {
                    format!("{}:{}", name, port)
                }
            })
            .collect();

        let header_text = vec![Line::from(vec![
            Span::styled(
                format!(" 🚀 Lambdaform v{} ", app.server_info.version),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("│ "),
            Span::styled(ports_str.join(", "), Style::default().fg(Color::Green)),
            Span::raw(" │ "),
            Span::styled(
                format!("{} fns", app.server_info.functions.len()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" │ ⏱ "),
            Span::styled(uptime_str, Style::default().fg(Color::White)),
            if app.server_info.watching {
                Span::styled(" │ 👀 watching", Style::default().fg(Color::Magenta))
            } else {
                Span::raw("")
            },
        ])];

        let header = Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        f.render_widget(header, area);
    }

    fn draw_request_table(f: &mut Frame, app: &mut App, area: Rect) {
        let header_cells = [
            "Time", "Method", "Path", "Status", "Duration", "Function", "Size",
        ]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        });
        let header = Row::new(header_cells).height(1);

        let rows: Vec<Row> = app
            .requests
            .iter()
            .map(|req| {
                let status_style = if req.status < 300 {
                    Style::default().fg(Color::Green)
                } else if req.status < 400 {
                    Style::default().fg(Color::Yellow)
                } else if req.status < 500 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                };

                let method_style = match req.method.as_str() {
                    "GET" => Style::default().fg(Color::Cyan),
                    "POST" => Style::default().fg(Color::Green),
                    "PUT" => Style::default().fg(Color::Yellow),
                    "DELETE" => Style::default().fg(Color::Red),
                    _ => Style::default().fg(Color::White),
                };

                let duration_str = if req.duration_ms < 1000 {
                    format!("{}ms", req.duration_ms)
                } else {
                    format!("{:.1}s", req.duration_ms as f64 / 1000.0)
                };

                let duration_style = if req.duration_ms < 200 {
                    Style::default().fg(Color::Green)
                } else if req.duration_ms < 1000 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Red)
                };

                let size_str = if req.response_bytes < 1024 {
                    format!("{}B", req.response_bytes)
                } else {
                    format!("{:.1}KB", req.response_bytes as f64 / 1024.0)
                };

                // Extract just time portion from timestamp
                let time = if req.timestamp.len() > 11 {
                    &req.timestamp[11..19]
                } else {
                    &req.timestamp
                };

                Row::new(vec![
                    Cell::from(time.to_string()),
                    Cell::from(req.method.clone()).style(method_style),
                    Cell::from(truncate_path(&req.path, 40)),
                    Cell::from(req.status.to_string()).style(status_style),
                    Cell::from(duration_str).style(duration_style),
                    Cell::from(truncate_path(&req.function, 20)),
                    Cell::from(size_str),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(8),  // Time
                Constraint::Length(7),  // Method
                Constraint::Min(20),    // Path
                Constraint::Length(6),  // Status
                Constraint::Length(8),  // Duration
                Constraint::Length(20), // Function
                Constraint::Length(8),  // Size
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(" Request Log ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

        f.render_stateful_widget(table, area, &mut app.table_state);
    }

    fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
        let error_rate = if app.total_requests > 0 {
            (app.total_errors as f64 / app.total_requests as f64) * 100.0
        } else {
            0.0
        };

        let scroll_indicator = if app.scroll_locked { "LIVE" } else { "SCROLL" };

        let status = Line::from(vec![
            Span::styled(
                format!(" {} ", scroll_indicator),
                Style::default()
                    .bg(if app.scroll_locked {
                        Color::Green
                    } else {
                        Color::Yellow
                    })
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " {} requests │ {} errors ({:.0}%) │ ",
                app.total_requests, app.total_errors, error_rate
            )),
            Span::styled(
                "q:quit  ↑↓:scroll  G:bottom  c:clear",
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        f.render_widget(Paragraph::new(status), area);
    }

    fn truncate_path(s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            format!("…{}", &s[s.len() - max + 1..])
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn make_event(method: &str, path: &str, status: u16, duration_ms: u64) -> RequestEvent {
            RequestEvent {
                timestamp: "2026-03-18T01:00:00Z".to_string(),
                method: method.to_string(),
                path: path.to_string(),
                status,
                duration_ms,
                function: "test-fn".to_string(),
                response_bytes: 256,
            }
        }

        fn make_server_info() -> ServerInfo {
            ServerInfo {
                version: "1.0.3".to_string(),
                ports: vec![("api".to_string(), 3000)],
                functions: vec!["hello".to_string()],
                watching: true,
            }
        }

        // --- truncate_path tests ---

        #[test]
        fn test_truncate_path_short() {
            assert_eq!(truncate_path("/api/hello", 40), "/api/hello");
        }

        #[test]
        fn test_truncate_path_exact() {
            let s = "x".repeat(40);
            assert_eq!(truncate_path(&s, 40), s);
        }

        #[test]
        fn test_truncate_path_long() {
            let s = "/api/v1/users/12345/orders/67890/items";
            let result = truncate_path(s, 20);
            assert!(result.starts_with('…'));
            // '…' is 3 bytes UTF-8, plus (max-1) bytes of the tail = 22 bytes total
            assert_eq!(result.chars().count(), 20); // 20 characters
                                                    // Verify it took the tail of the original string
            assert!(s.ends_with(&result[3..])); // skip '…' bytes
        }

        #[test]
        fn test_truncate_path_empty() {
            assert_eq!(truncate_path("", 10), "");
        }

        // --- App::push_event tests ---

        #[test]
        fn test_push_event_increments_total() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/", 200, 5));
            assert_eq!(app.total_requests, 1);
            assert_eq!(app.total_errors, 0);
            assert_eq!(app.requests.len(), 1);
        }

        #[test]
        fn test_push_event_counts_errors() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/ok", 200, 5));
            app.push_event(make_event("GET", "/bad", 400, 5));
            app.push_event(make_event("GET", "/err", 500, 5));
            app.push_event(make_event("GET", "/redirect", 302, 5));
            assert_eq!(app.total_requests, 4);
            assert_eq!(app.total_errors, 2); // 400 + 500
        }

        #[test]
        fn test_push_event_buffer_eviction() {
            let mut app = App::new(make_server_info());
            // Push 501 events — should evict first 100 when it hits 501
            for i in 0..501 {
                app.push_event(make_event("GET", &format!("/{}", i), 200, 1));
            }
            assert_eq!(app.total_requests, 501);
            // Buffer should have been trimmed: 501 pushed, then drain(..100) → 401 remain
            assert_eq!(app.requests.len(), 401);
            // First remaining event should be /100 (0-99 were drained)
            assert_eq!(app.requests[0].path, "/100");
        }

        #[test]
        fn test_push_event_auto_scrolls_when_locked() {
            let mut app = App::new(make_server_info());
            assert!(app.scroll_locked);
            app.push_event(make_event("GET", "/1", 200, 5));
            assert_eq!(app.table_state.selected(), Some(0));
            app.push_event(make_event("GET", "/2", 200, 5));
            assert_eq!(app.table_state.selected(), Some(1));
        }

        // --- Scroll behavior tests ---

        #[test]
        fn test_scroll_up_unlocks() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/1", 200, 5));
            app.push_event(make_event("GET", "/2", 200, 5));
            app.push_event(make_event("GET", "/3", 200, 5));
            assert!(app.scroll_locked);
            assert_eq!(app.table_state.selected(), Some(2));

            app.scroll_up();
            assert!(!app.scroll_locked);
            assert_eq!(app.table_state.selected(), Some(1));
        }

        #[test]
        fn test_scroll_up_stops_at_zero() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/1", 200, 5));
            app.scroll_locked = false;
            app.table_state.select(Some(0));
            app.scroll_up();
            assert_eq!(app.table_state.selected(), Some(0));
        }

        #[test]
        fn test_scroll_down_relocks_at_bottom() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/1", 200, 5));
            app.push_event(make_event("GET", "/2", 200, 5));
            app.push_event(make_event("GET", "/3", 200, 5));

            // Manually unlock and go to middle
            app.scroll_locked = false;
            app.table_state.select(Some(1));

            app.scroll_down(); // → 2 (last item)
            assert!(app.scroll_locked); // should re-lock
            assert_eq!(app.table_state.selected(), Some(2));
        }

        #[test]
        fn test_scroll_down_empty() {
            let mut app = App::new(make_server_info());
            app.scroll_down(); // should not panic
            assert_eq!(app.table_state.selected(), None);
        }

        #[test]
        fn test_scroll_to_bottom() {
            let mut app = App::new(make_server_info());
            for i in 0..10 {
                app.push_event(make_event("GET", &format!("/{}", i), 200, 1));
            }
            app.scroll_locked = false;
            app.table_state.select(Some(3));

            app.scroll_to_bottom();
            assert!(app.scroll_locked);
            assert_eq!(app.table_state.selected(), Some(9));
        }

        #[test]
        fn test_scroll_up_with_no_selection() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/1", 200, 5));
            app.push_event(make_event("GET", "/2", 200, 5));
            app.table_state.select(None);
            app.scroll_up();
            // Should select last item when no selection
            assert_eq!(app.table_state.selected(), Some(1));
        }

        #[test]
        fn test_scroll_down_with_no_selection() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/1", 200, 5));
            app.table_state.select(None);
            app.scroll_down();
            assert_eq!(app.table_state.selected(), Some(0));
        }

        // --- App initialization ---

        #[test]
        fn test_app_initial_state() {
            let app = App::new(make_server_info());
            assert!(app.scroll_locked);
            assert_eq!(app.total_requests, 0);
            assert_eq!(app.total_errors, 0);
            assert!(app.requests.is_empty());
            assert_eq!(app.table_state.selected(), None);
            assert_eq!(app.server_info.version, "1.0.3");
        }

        #[test]
        fn test_error_rate_boundary_399() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/", 399, 5));
            assert_eq!(app.total_errors, 0); // 399 is not an error
        }

        #[test]
        fn test_error_rate_boundary_400() {
            let mut app = App::new(make_server_info());
            app.push_event(make_event("GET", "/", 400, 5));
            assert_eq!(app.total_errors, 1); // 400 IS an error
        }
    }
}

/// Create a TUI event channel. Returns (sender, receiver).
/// The sender should be shared with the server's request handler.
#[cfg(feature = "tui")]
pub fn create_tui_channel() -> (
    tokio::sync::broadcast::Sender<ui::RequestEvent>,
    tokio::sync::broadcast::Receiver<ui::RequestEvent>,
) {
    tokio::sync::broadcast::channel(256)
}

/// Stub for non-TUI builds
#[cfg(not(feature = "tui"))]
pub fn tui_not_available() {
    eprintln!("Error: TUI feature not enabled. Rebuild with: cargo build --features tui");
    std::process::exit(1);
}
