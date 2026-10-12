use crate::{Position, cell};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::layout::Constraint;
use ratatui::prelude::Stylize;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Row, Table, TableState};
use std::io;

///open full-screen view and close with q
pub fn run(positions: &[Position]) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, positions);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, positions: &[Position]) -> io::Result<()> {
    let mut state = TableState::default().with_selected(0);
    loop {
        terminal.draw(|frame| {
            frame.render_stateful_widget(table(positions), frame.area(), &mut state);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
                _ => {}
            }
        }
    }
}

/// A table cell with its text pushed to the right edge, as numbers should be.
fn right<'a>(text: impl Into<Line<'a>>) -> Cell<'a> {
    Cell::from(text.into().right_aligned())
}

/// Builds the positions table: one line per position under a header.
fn table(positions: &[Position]) -> Table<'_> {
    let header = Row::new([
        Cell::from("name"),
        Cell::from("code"),
        right("shares"),
        right("cost"),
        right("avg"),
        right("realized"),
        right("value"),
        right("unrealized"),
        right("dividends"),
    ]);

    let rows = positions.iter().map(|position| {
        let holding = &position.holding;
        Row::new([
            Cell::from(holding.name.as_str()),
            Cell::from(position.isin.as_str()),
            right(holding.shares().normalize().to_string()),
            right(cell(Some(holding.cost_basis()))),
            right(cell(holding.avg_price())),
            right(cell(Some(holding.realized))),
            right(cell(position.value())),
            right(cell(position.unrealized())),
            right(cell(Some(holding.dividends))),
        ])
    });

    let widths = [
        Constraint::Length(32),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title("tradetrack: press q to quit"))
        .block(Block::bordered().title("tradetrack: up/down to move, q to quit"))
        .row_highlight_style(Style::new().reversed())
}
