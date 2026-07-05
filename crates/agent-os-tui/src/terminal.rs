use crate::{all_commands, default_keymap, timeline_lines, TuiApp, TuiAppClient, TuiExitReport};
use agent_os_sys::{AgentOsError, AgentOsResult};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Terminal,
};
use std::io::{self, Stdout};
use std::time::Duration;

pub fn run<C: TuiAppClient>(app: &mut TuiApp<C>) -> AgentOsResult<TuiExitReport> {
    app.initialize()?;
    enable_raw_mode().map_err(terminal_error("enable raw mode"))?;
    let mut stdout = io::stdout();
    if !app.options.no_alt_screen {
        execute!(stdout, EnterAlternateScreen).map_err(terminal_error("enter alternate screen"))?;
    }
    let mut terminal =
        Terminal::new(CrosstermBackend::new(stdout)).map_err(terminal_error("create terminal"))?;
    let result = run_loop(app, &mut terminal);
    let cleanup = cleanup_terminal(&mut terminal, app.options.no_alt_screen);
    result?;
    cleanup?;
    Ok(app.exit_report())
}

fn run_loop<C: TuiAppClient>(
    app: &mut TuiApp<C>,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> AgentOsResult<()> {
    while !app.should_exit {
        terminal
            .draw(|frame| render(frame, app))
            .map_err(terminal_error("draw terminal"))?;
        if event::poll(Duration::from_millis(150)).map_err(terminal_error("poll terminal event"))? {
            if let Event::Key(key) = event::read().map_err(terminal_error("read terminal event"))? {
                handle_key(app, key)?;
            }
        }
        app.drain_notifications()?;
    }
    Ok(())
}

fn cleanup_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    no_alt_screen: bool,
) -> AgentOsResult<()> {
    disable_raw_mode().map_err(terminal_error("disable raw mode"))?;
    if !no_alt_screen {
        execute!(terminal.backend_mut(), LeaveAlternateScreen)
            .map_err(terminal_error("leave alternate screen"))?;
    }
    terminal
        .show_cursor()
        .map_err(terminal_error("show terminal cursor"))?;
    Ok(())
}

fn handle_key<C: TuiAppClient>(app: &mut TuiApp<C>, key: KeyEvent) -> AgentOsResult<()> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            if app.projection.running() {
                app.handle_input("/interrupt")?;
            } else if app.composer.is_empty() {
                app.should_exit = true;
            } else {
                app.composer.clear();
            }
        }
        (KeyCode::Esc, _) => app.close_top_mode(),
        (KeyCode::F(1), _) => app.handle_input("/help")?,
        (KeyCode::Enter, _) => app.submit_composer()?,
        (KeyCode::Backspace, _) => app.composer.backspace(),
        (KeyCode::Char(ch), KeyModifiers::NONE | KeyModifiers::SHIFT) => app.composer.push(ch),
        _ => {}
    }
    Ok(())
}

fn render<C: TuiAppClient>(frame: &mut ratatui::Frame<'_>, app: &TuiApp<C>) {
    let bottom_height = if app.bottom_pane.is_some() { 6 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(bottom_height),
            Constraint::Length(3),
        ])
        .split(frame.area());
    render_status(frame, chunks[0], app);
    render_timeline(frame, chunks[1], app);
    if app.bottom_pane.is_some() {
        render_bottom_pane(frame, chunks[2], app);
    }
    render_composer(frame, chunks[3], app);
    if app.overlay.is_some() {
        render_overlay(frame, centered_rect(frame.area(), 80, 75), app);
    }
}

fn render_status<C: TuiAppClient>(frame: &mut ratatui::Frame<'_>, area: Rect, app: &TuiApp<C>) {
    let thread = app
        .projection
        .current_thread_id
        .as_deref()
        .unwrap_or("no-thread");
    let status = app.projection.thread_status.as_deref().unwrap_or("ready");
    let model = app.options.model.as_deref().unwrap_or("default-model");
    let text = Line::from(vec![
        Span::styled(
            " Agent-OS ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(format!(" {thread}  {status}  {model}  ")),
        Span::styled(&app.status_line, Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(text), area);
}

fn render_timeline<C: TuiAppClient>(frame: &mut ratatui::Frame<'_>, area: Rect, app: &TuiApp<C>) {
    let height = area.height.saturating_sub(2) as usize;
    let lines = timeline_lines(&app.projection, height)
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();
    let block = Block::default().borders(Borders::ALL).title("Timeline");
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_bottom_pane<C: TuiAppClient>(
    frame: &mut ratatui::Frame<'_>,
    area: Rect,
    app: &TuiApp<C>,
) {
    let Some(pane) = app.bottom_pane else {
        return;
    };
    let text = match pane.title() {
        "Threads" => vec![Line::from(
            "Use /threads to refresh and /resume <thread-id> to switch.",
        )],
        "Models" => vec![Line::from(format!(
            "model={} profile={}",
            app.options.model.as_deref().unwrap_or("default"),
            app.options.profile.as_deref().unwrap_or("default")
        ))],
        "Processes" => app
            .projection
            .process_sessions
            .iter()
            .map(|process| Line::from(process.to_string()))
            .collect(),
        _ => vec![Line::from(format!(
            "thread={:?} turn={:?} runtime_jobs={}",
            app.projection.current_thread_id,
            app.projection.current_turn_id,
            app.projection.runtime_jobs.len()
        ))],
    };
    let block = Block::default().borders(Borders::ALL).title(pane.title());
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

fn render_composer<C: TuiAppClient>(frame: &mut ratatui::Frame<'_>, area: Rect, app: &TuiApp<C>) {
    let title = if app.projection.running() {
        "Composer: steering input"
    } else {
        "Composer"
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(
        Paragraph::new(app.composer.text.as_str()).block(block),
        area,
    );
}

fn render_overlay<C: TuiAppClient>(frame: &mut ratatui::Frame<'_>, area: Rect, app: &TuiApp<C>) {
    let Some(overlay) = app.overlay else {
        return;
    };
    frame.render_widget(Clear, area);
    let lines = match overlay.title() {
        "Help" => all_commands()
            .iter()
            .map(|command| {
                Line::from(vec![
                    Span::styled(
                        format!("/{:<12}", command.slash),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(command.description),
                ])
            })
            .collect(),
        "Keymap" => default_keymap()
            .iter()
            .map(|binding| Line::from(format!("{}  {}", binding.key, binding.description)))
            .collect(),
        _ => vec![
            Line::from(format!("timeline items: {}", app.projection.timeline.len())),
            Line::from(format!(
                "runtime jobs: {}",
                app.projection.runtime_jobs.len()
            )),
            Line::from(format!("artifacts: {}", app.projection.artifacts.len())),
            Line::from(format!("evidence: {}", app.projection.evidence.len())),
            Line::from(format!("resources: {}", app.projection.resources.len())),
            Line::from(format!(
                "automation runs: {}",
                app.projection.automation_runs.len()
            )),
            Line::from(raw_projection_text(app)),
        ],
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(overlay.title());
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn raw_projection_text<C: TuiAppClient>(app: &TuiApp<C>) -> String {
    if !app.raw_mode {
        return "raw projection hidden; use /raw on".to_string();
    }
    app.projection
        .raw
        .as_ref()
        .and_then(|value| serde_json::to_string_pretty(value).ok())
        .unwrap_or_else(|| "no raw projection loaded".to_string())
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn terminal_error(context: &'static str) -> impl FnOnce(io::Error) -> AgentOsError {
    move |error| AgentOsError::Validation(format!("{context}: {error}"))
}
