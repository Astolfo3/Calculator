use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame, Terminal,
};

// ─── pastel aesthetic colors ──────────────────────────────────────────────────

const PINK_LIGHT: Color = Color::Rgb(255, 183, 201);   // #FFB7C9  — soft blush
const PINK_MED: Color = Color::Rgb(255, 143, 171);    // #FF8FAB  — rose
const PINK_DEEP: Color = Color::Rgb(255, 107, 145);   // #FF6B91  — deep rose
const LAVENDER: Color = Color::Rgb(201, 182, 255);    // #C9B6FF  — soft lavender
const MINT: Color = Color::Rgb(182, 255, 201);        // #B6FFC9  — soft mint
const BG_DARK: Color = Color::Rgb(26, 21, 32);        // #1A1520  — very dark purple
const BG_SURFACE: Color = Color::Rgb(42, 32, 53);     // #2A2035  — dark purple surface
const TEXT_SOFT: Color = Color::Rgb(255, 228, 236);   // #FFE4EC  — light pink text
const TEXT_WHITE: Color = Color::Rgb(255, 248, 250);  // #FFF8FA  — warm white

// ─── Calculator operation ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

impl Op {
    const fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "−",
            Op::Mul => "×",
            Op::Div => "÷",
        }
    }
}

// ─── Calculator state ───────────────────────────────────────────────────────────

struct Calc {
    display: String,      // full expression shown on screen
    current: String,      // current number being typed
    previous: Option<f64>, // previous operand (if an op was entered)
    operation: Option<Op>, // pending operation
    reset_next: bool,     // next digit should start fresh
    error: Option<String>,
}

impl Calc {
    fn new() -> Self {
        Self {
            display: String::new(),
            current: "0".into(),
            previous: None,
            operation: None,
            reset_next: false,
            error: None,
        }
    }

    // ── digit / decimal input ───────────────────────────────────────────────

    fn input_digit(&mut self, d: char) {
        self.error = None;
        if self.reset_next {
            self.current.clear();
            self.reset_next = false;
        }
        if self.current == "0" {
            self.current.clear();
        }
        self.current.push(d);
        self.sync_display();
    }

    fn input_decimal(&mut self) {
        self.error = None;
        if self.reset_next {
            self.current.clear();
            self.reset_next = false;
        }
        if self.current.is_empty() {
            self.current.push('0');
        }
        if !self.current.contains('.') {
            self.current.push('.');
        }
        self.sync_display();
    }

    // ── operations ─────────────────────────────────────────────────────────

    fn set_op(&mut self, op: Op) {
        self.error = None;
        // if there's already a pending operation, evaluate it first :3
        if self.operation.is_some() {
            self.evaluate();
        }
        if let Ok(val) = self.current.parse::<f64>() {
            self.previous = Some(val);
        }
        self.operation = Some(op);
        self.reset_next = true;
        self.sync_display();
    }

    fn evaluate(&mut self) -> Option<f64> {
        let current_val = self.current.parse::<f64>().ok()?;
        let previous_val = self.previous?;
        let op = self.operation?;

        let result = match op {
            Op::Add => previous_val + current_val,
            Op::Sub => previous_val - current_val,
            Op::Mul => previous_val * current_val,
            Op::Div => {
                if current_val == 0.0 {
                    self.error = Some("division by zero 💔".into());
                    return None;
                }
                previous_val / current_val
            }
        };

        self.current = fmt_number(result);
        self.previous = None;
        self.operation = None;
        self.reset_next = true;
        self.sync_display();
        Some(result)
    }

    fn equals(&mut self) {
        self.error = None;
        self.evaluate();
    }

    // ── utility functions ──────────────────────────────────────────────────

    fn clear(&mut self) {
        self.current = "0".into();
        self.previous = None;
        self.operation = None;
        self.reset_next = false;
        self.error = None;
        self.sync_display();
    }

    fn backspace(&mut self) {
        if self.current.len() > 1 {
            self.current.pop();
        } else {
            self.current = "0".into();
        }
        self.sync_display();
    }

    fn negate(&mut self) {
        if let Ok(val) = self.current.parse::<f64>() {
            let neg = -val;
            self.current = fmt_number(neg);
            self.sync_display();
        }
    }

    fn percent(&mut self) {
        if let Ok(val) = self.current.parse::<f64>() {
            let pct = val / 100.0;
            self.current = fmt_number(pct);
            self.sync_display();
        }
    }

    fn sync_display(&mut self) {
        let mut parts: Vec<String> = Vec::new();
        if let Some(prev) = self.previous {
            parts.push(fmt_number(prev));
        }
        if let Some(op) = self.operation {
            parts.push(op.symbol().to_string());
        }
        parts.push(self.current.clone());
        self.display = if parts.is_empty() {
            self.current.clone()
        } else {
            parts.join(" ")
        };
    }
}

/// format an f64 nicely — no trailing ".0" for integers :3
fn fmt_number(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() {
        format!("{:.0}", n)
    } else {
        format!("{}", n)
    }
}

// ─── TUI rendering ─────────────────────────────────────────────────────────────

/// label + (row, col, col_span) for a button in a 5‑column grid.
const KEYBOARD: &[(&str, u16, u16, u16)] = &[
    ("7", 0, 0, 1),
    ("8", 0, 1, 1),
    ("9", 0, 2, 1),
    ("÷", 0, 3, 1),
    ("C", 0, 4, 1),
    ("4", 1, 0, 1),
    ("5", 1, 1, 1),
    ("6", 1, 2, 1),
    ("×", 1, 3, 1),
    ("±", 1, 4, 1),
    ("1", 2, 0, 1),
    ("2", 2, 1, 1),
    ("3", 2, 2, 1),
    ("−", 2, 3, 1),
    ("%", 2, 4, 1),
    ("0", 3, 0, 2),
    (".", 3, 2, 1),
    ("=", 3, 3, 1),
    ("+", 3, 4, 1),
];

fn draw_buttons(frame: &mut Frame, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    for &(label, row, col, colspan) in KEYBOARD {
        let row_area = rows[row as usize];
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(1, 5),
                Constraint::Ratio(1, 5),
                Constraint::Ratio(1, 5),
                Constraint::Ratio(1, 5),
                Constraint::Ratio(1, 5),
            ])
            .split(row_area);

        // merge columns for colspan
        let mut btn_area = cols[col as usize];
        for i in 1..colspan {
            btn_area = Rect {
                width: btn_area.width + cols[(col + i) as usize].width,
                ..btn_area
            };
        }

        // pick a cute color per button type :3
        let (bg, fg) = if label == "C" {
            (PINK_DEEP, TEXT_WHITE)
        } else if label == "=" {
            (PINK_MED, TEXT_WHITE)
        } else if matches!(label, "÷" | "×" | "−" | "+") {
            (LAVENDER, BG_DARK)
        } else if matches!(label, "±" | "%") {
            (MINT, BG_DARK)
        } else {
            (PINK_LIGHT, BG_DARK)
        };

        let btn_border = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(fg).bg(bg));

        let inner = btn_border.inner(btn_area);
        frame.render_widget(btn_border, btn_area);

        let btn_text = Paragraph::new(Text::styled(
            label,
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center);
        frame.render_widget(btn_text, inner);
    }
}

fn draw_ui(frame: &mut Frame, calc: &Calc) {
    let area = frame.size();

    // ── fill background ───────────────────────────────────────────────────
    let bg_block = Block::default().style(Style::default().bg(BG_DARK));
    frame.render_widget(bg_block, area);

    // ── outer border ──────────────────────────────────────────────────────
    let outer = Block::default()
        .title(" 🌸  calculator  🌸 ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().fg(PINK_LIGHT).bg(BG_DARK));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // ── layout ────────────────────────────────────────────────────────────
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // display
            Constraint::Min(0),     // buttons
            Constraint::Length(1),  // hint bar
        ])
        .horizontal_margin(2)
        .split(inner);

    // ── display area ──────────────────────────────────────────────────────
    let (display_fg, display_bg) = if calc.error.is_some() {
        (PINK_DEEP, BG_SURFACE)
    } else {
        (PINK_LIGHT, BG_SURFACE)
    };
    let display_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().fg(display_fg).bg(display_bg));
    let display_inner = display_block.inner(chunks[0]);
    frame.render_widget(display_block, chunks[0]);

    let display_text = match &calc.error {
        Some(e) => format!("💔  {}", e),
        None => calc.display.clone(),
    };
    let display_color = if calc.error.is_some() {
        PINK_DEEP
    } else {
        TEXT_SOFT
    };
    let display_para = Paragraph::new(Text::styled(
        &display_text,
        Style::default().fg(display_color).add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Right)
    .wrap(Wrap { trim: false });
    frame.render_widget(display_para, display_inner);

    // ── button grid ───────────────────────────────────────────────────────
    draw_buttons(frame, chunks[1]);

    // ── cute hint bar ─────────────────────────────────────────────────────
    let hint = Paragraph::new(Line::from(vec![
        Span::styled(" q ", Style::default().fg(TEXT_WHITE).bg(PINK_MED)),
        Span::raw(" quit  "),
        Span::styled(" c ", Style::default().fg(TEXT_WHITE).bg(PINK_MED)),
        Span::raw(" clear  "),
        Span::styled(" ← ", Style::default().fg(TEXT_WHITE).bg(PINK_MED)),
        Span::raw(" backspace  "),
        Span::styled(" n ", Style::default().fg(TEXT_WHITE).bg(LAVENDER)),
        Span::raw(" negate  "),
        Span::styled(" % ", Style::default().fg(TEXT_WHITE).bg(MINT)),
        Span::raw(" percent"),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().fg(TEXT_SOFT).bg(BG_DARK));
    frame.render_widget(hint, chunks[2]);
}

// ─── Main event loop ───────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut calc = Calc::new();

    // Restore terminal even on panic
    let res = run_app(&mut terminal, &mut calc);

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, calc: &mut Calc) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, calc))?;

        // Poll for key with a timeout so we can redraw periodically if needed
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                    if handle_key(key.code, calc) {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Returns `true` when the application should quit.
fn handle_key(code: KeyCode, calc: &mut Calc) -> bool {
    match code {
        // Quit
        KeyCode::Esc | KeyCode::Char('q') => return true,

        // Clear
        KeyCode::Char('c') | KeyCode::Char('C') => calc.clear(),

        // Backspace
        KeyCode::Backspace => calc.backspace(),

        // Negate
        KeyCode::Char('n') | KeyCode::Char('N') => calc.negate(),

        // Percent
        KeyCode::Char('%') => calc.percent(),

        // Digits
        KeyCode::Char(ch) if ch.is_ascii_digit() => calc.input_digit(ch),

        // Decimal
        KeyCode::Char('.') | KeyCode::Char(',') => calc.input_decimal(),

        // Operations
        KeyCode::Char('+') => calc.set_op(Op::Add),
        KeyCode::Char('-') => calc.set_op(Op::Sub),
        KeyCode::Char('*') => calc.set_op(Op::Mul),
        KeyCode::Char('/') => calc.set_op(Op::Div),

        // Equals
        KeyCode::Enter | KeyCode::Char('=') => calc.equals(),

        // Arrow‑based navigation (also map these)
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {}

        _ => {}
    }
    false
}
