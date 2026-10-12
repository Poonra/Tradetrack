use crate::{Position, Summary, cell, summary};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Cell, Row, Table, TableState, Tabs};
use rust_decimal::Decimal;
use std::io;
use tradetrack::Sale;

const ORANGE: Color = Color::Indexed(208);

/// How many rows Page Up and Page Down move the selection.
const PAGE_STEP: u16 = 20;

/// The pages of the screen, in the order of the tab bar.
#[derive(Clone, Copy)]
enum Page {
    Holdings,
    Sold,
    Transactions,
}

impl Page {
    const TITLES: [&str; 3] = ["Holdings", "Sold", "Transactions"];

    fn next(self) -> Self {
        match self {
            Self::Holdings => Self::Sold,
            Self::Sold => Self::Transactions,
            Self::Transactions => Self::Holdings,
        }
    }
}

/// Opens the full-screen view and keeps it open until `q` is pressed.
pub fn run(
    positions: &[Position],
    rows: &[tradetrack::Row],
    total_return: Option<Decimal>,
) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, positions, rows, total_return);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut DefaultTerminal,
    positions: &[Position],
    rows: &[tradetrack::Row],
    total_return: Option<Decimal>,
) -> io::Result<()> {
    let open: Vec<&Position> = positions
        .iter()
        .filter(|position| !position.holding.shares().is_zero())
        .collect();
    let open_summary = summary(&open);

    // Newest first. The timestamps are ISO 8601 text, which sorts in time order.
    let mut ledger: Vec<&tradetrack::Row> = rows.iter().collect();
    ledger.sort_by(|a, b| b.datetime.cmp(&a.datetime));

    // The years that occur in the export, newest first. `None` shows every year.
    let mut years: Vec<&str> = ledger.iter().map(|row| year(&row.date)).collect();
    years.dedup();
    let mut filter: Option<&str> = None;

    let mut page = Page::Holdings;
    let mut open_state = TableState::default().with_selected(0);
    let mut sold_state = TableState::default().with_selected(0);
    let mut ledger_state = TableState::default().with_selected(0);

    loop {
        let visible: Vec<&tradetrack::Row> = ledger
            .iter()
            .copied()
            .filter(|row| in_period(&row.date, filter))
            .collect();
        let sold = sold_lines(positions, filter);

        terminal.draw(|frame| {
            let [tabs_area, table_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(frame.area());

            frame.render_widget(Tabs::new(Page::TITLES).select(page as usize), tabs_area);

            let block = Block::bordered()
                .title("tab: switch page, up/down/pgup/pgdn/home/end: move, q: quit")
                .title_bottom(format!("total return after tax: {}", cell(total_return)));
            let hint = Line::from(format!("f: year [{}]", filter.unwrap_or("all"))).right_aligned();

            match page {
                Page::Holdings => frame.render_stateful_widget(
                    holdings_table(&open, &open_summary).block(block),
                    table_area,
                    &mut open_state,
                ),
                Page::Sold => frame.render_stateful_widget(
                    sold_table(&sold).block(block.title_top(hint)),
                    table_area,
                    &mut sold_state,
                ),
                Page::Transactions => frame.render_stateful_widget(
                    transactions_table(&visible).block(block.title_top(hint)),
                    table_area,
                    &mut ledger_state,
                ),
            }
        })?;

        if let Event::Key(key) = event::read()? {
            let state = match page {
                Page::Holdings => &mut open_state,
                Page::Sold => &mut sold_state,
                Page::Transactions => &mut ledger_state,
            };
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Tab => page = page.next(),
                KeyCode::Char('1') => page = Page::Holdings,
                KeyCode::Char('2') => page = Page::Sold,
                KeyCode::Char('3') => page = Page::Transactions,
                KeyCode::Char('f') if matches!(page, Page::Sold | Page::Transactions) => {
                    filter = match filter {
                        None => years.first().copied(),
                        Some(current) => years
                            .iter()
                            .copied()
                            .skip_while(|year| *year != current)
                            .nth(1),
                    };
                    state.select_first();
                }
                KeyCode::Down | KeyCode::Char('j') => state.select_next(),
                KeyCode::Up | KeyCode::Char('k') => state.select_previous(),
                KeyCode::PageDown => state.scroll_down_by(PAGE_STEP),
                KeyCode::PageUp => state.scroll_up_by(PAGE_STEP),
                KeyCode::Home => state.select_first(),
                KeyCode::End => state.select_last(),
                _ => {}
            }
        }
    }
}

/// A table cell with its text pushed to the right edge, as numbers should be.
fn right<'a>(text: impl Into<Line<'a>>) -> Cell<'a> {
    Cell::from(text.into().right_aligned())
}

/// A right-aligned amount in one colour for a gain and another for a loss.
fn signed(amount: Option<Decimal>, gain: Color, loss: Color) -> Cell<'static> {
    let color = match amount {
        Some(amount) if amount > Decimal::ZERO => gain,
        Some(amount) if amount < Decimal::ZERO => loss,
        _ => Color::Reset,
    };
    right(cell(amount)).style(Style::new().fg(color))
}

/// The positions still held: one line each between a header and a totals line.
fn holdings_table<'a>(positions: &[&'a Position], summary: &Summary) -> Table<'a> {
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

    let rows = positions.iter().copied().map(|position| {
        let holding = &position.holding;
        Row::new([
            Cell::from(holding.name.as_str()),
            Cell::from(position.isin.as_str()),
            right(holding.shares().normalize().to_string()),
            right(cell(Some(holding.cost_basis()))),
            right(cell(holding.avg_price())),
            signed(Some(holding.realized), Color::Green, Color::Red),
            right(cell(position.value())),
            signed(position.unrealized(), Color::Yellow, ORANGE),
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

    let footer = Row::new([
        Cell::from("TOTAL"),
        Cell::from(""),
        Cell::from(""),
        right(cell(Some(summary.cost))),
        Cell::from(""),
        signed(Some(summary.realized), Color::Green, Color::Red),
        right(cell(summary.value)),
        signed(summary.unrealized, Color::Yellow, ORANGE),
        right(cell(Some(summary.dividends))),
    ]);

    Table::new(rows, widths)
        .header(header)
        .footer(footer)
        .row_highlight_style(Style::new().reversed())
}

/// One line of the Sold page: what a position sold and earned in the chosen period.
struct SoldLine<'a> {
    position: &'a Position,
    sold: Decimal,
    realized: Decimal,
    dividends: Decimal,
}

/// Adds up each position's sales and dividends of the chosen period.
///
/// A position without a sale in that period is left out. A partly sold position is
/// therefore on both pages: what is left on Holdings, what was sold here.
fn sold_lines<'a>(positions: &'a [Position], filter: Option<&str>) -> Vec<SoldLine<'a>> {
    positions
        .iter()
        .filter_map(|position| {
            let holding = &position.holding;
            let sales: Vec<&Sale> = holding
                .sales
                .iter()
                .filter(|sale| in_period(&sale.date, filter))
                .collect();
            if sales.is_empty() {
                return None;
            }
            Some(SoldLine {
                position,
                sold: sales.iter().map(|sale| sale.shares).sum(),
                realized: sales.iter().map(|sale| sale.realized).sum(),
                dividends: holding
                    .payouts
                    .iter()
                    .filter(|payout| in_period(&payout.date, filter))
                    .map(|payout| payout.net)
                    .sum(),
            })
        })
        .collect()
}

/// The Sold page: one line per position sold in the period, above a totals line.
fn sold_table<'a>(lines: &[SoldLine<'a>]) -> Table<'a> {
    let header = Row::new([
        Cell::from("name"),
        Cell::from("code"),
        right("sold"),
        right("still held"),
        right("realized"),
        right("dividends"),
    ]);

    let rows = lines.iter().map(|line| {
        let holding = &line.position.holding;
        Row::new([
            Cell::from(holding.name.as_str()),
            Cell::from(line.position.isin.as_str()),
            right(line.sold.normalize().to_string()),
            right(holding.shares().normalize().to_string()),
            signed(Some(line.realized), Color::Green, Color::Red),
            right(cell(Some(line.dividends))),
        ])
    });

    let widths = [
        Constraint::Length(32),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let realized: Decimal = lines.iter().map(|line| line.realized).sum();
    let dividends: Decimal = lines.iter().map(|line| line.dividends).sum();
    let footer = Row::new([
        Cell::from("TOTAL"),
        Cell::from(format!("positions: {}", lines.len())),
        Cell::from(""),
        Cell::from(""),
        signed(Some(realized), Color::Green, Color::Red),
        right(cell(Some(dividends))),
    ]);

    Table::new(rows, widths)
        .header(header)
        .footer(footer)
        .row_highlight_style(Style::new().reversed())
}

/// The year of a date written as `2026-10-07`: its first four characters.
fn year(date: &str) -> &str {
    date.get(..4).unwrap_or_default()
}

/// Whether a date falls in the chosen year. With no year chosen, every date does.
fn in_period(date: &str, filter: Option<&str>) -> bool {
    filter.is_none_or(|selected| year(date) == selected)
}

/// Formats an amount, or leaves the cell empty if this kind of row has none.
fn optional(amount: Option<Decimal>) -> String {
    amount.map(|amount| cell(Some(amount))).unwrap_or_default()
}

/// The given rows of the export as they are, above a line that adds them up.
fn transactions_table<'a>(ledger: &[&'a tradetrack::Row]) -> Table<'a> {
    let header = Row::new([
        Cell::from("date"),
        Cell::from("type"),
        Cell::from("name"),
        right("shares"),
        right("amount"),
        right("fee"),
        right("tax"),
    ]);

    let rows = ledger.iter().copied().map(|row| {
        let shares = row
            .shares
            .map(|shares| shares.normalize().to_string())
            .unwrap_or_default();
        Row::new([
            Cell::from(row.date.as_str()),
            Cell::from(row.kind.as_str()),
            Cell::from(row.name.as_str()),
            right(shares),
            signed(row.amount, Color::Green, Color::Red),
            right(optional(row.fee)),
            right(optional(row.tax)),
        ])
    });

    let widths = [
        Constraint::Length(10),
        Constraint::Length(30),
        Constraint::Length(32),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(8),
    ];

    let amount: Decimal = ledger.iter().filter_map(|row| row.amount).sum();
    let fee: Decimal = ledger.iter().filter_map(|row| row.fee).sum();
    let tax: Decimal = ledger.iter().filter_map(|row| row.tax).sum();
    let footer = Row::new([
        Cell::from("TOTAL"),
        Cell::from(format!("rows: {}", ledger.len())),
        Cell::from(""),
        Cell::from(""),
        signed(Some(amount), Color::Green, Color::Red),
        right(cell(Some(fee))),
        right(cell(Some(tax))),
    ]);

    Table::new(rows, widths)
        .header(header)
        .footer(footer)
        .row_highlight_style(Style::new().reversed())
}
