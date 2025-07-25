// Simplified main.rs for getting basic TUI running
// This creates a minimal working version to demonstrate the interface

use anyhow::Result;
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture},
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
  backend::CrosstermBackend,
  layout::{Alignment, Constraint, Direction, Layout},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Paragraph},
  Terminal,
};
use std::io::{self, stdout};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

fn main() -> Result<()> {
    // Terminal initialization
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let size = f.area();

            // Split the screen into areas
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Min(10),    // Main content
                    Constraint::Length(3),  // Footer
                ])
                .split(size);

            // Header
            let header = Paragraph::new("Spotify TUI - Minimal Demo Version")
                .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title("Header"));
            f.render_widget(header, chunks[0]);

            // Main content
            let content = vec![
                Line::from("Welcome to Spotify TUI!"),
                Line::from(""),
                Line::from("This is a minimal working version to demonstrate the interface."),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::Yellow);
                    Span::styled("Demo Mode - Spotify features disabled", Style::default().fg(Color::Red);
                ]),
                Line::from(""),
                Line::from("The original application features:"),
                Line::from("• Browse playlists and tracks"),
                Line::from("• Control playback"),
                Line::from("• Search for music"),
                Line::from("• Audio analysis visualization"),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Press ", Style::default();
                    Span::styled("'q'", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
                    Span::styled(" to quit", Style::default();
                ]),
            ];

            let main_content = Paragraph::new(content)
                .style(Style::default().fg(Color::White))
                .block(Block::default().borders(Borders::ALL).title("Main"));
            f.render_widget(main_content, chunks[1]);

            // Footer
            let footer = Paragraph::new("TODO: Implement full Spotify integration with rspotify 0.15")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title("Status"));
            f.render_widget(footer, chunks[2]);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    println!("Thanks for trying Spotify TUI!");
    println!("This was a minimal demo version.");
    println!("The full application with Spotify integration requires additional work to update to modern dependencies.");

    Ok(())
}