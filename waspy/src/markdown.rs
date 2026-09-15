//! Claude's answers come back as markdown. This prints them formatted for a terminal: bold, italics,
//! inline code, links, headings, bullet and numbered lists (nested, with hanging indents), quotes,
//! code blocks, rules and tables, word-wrapped to the window. Piped somewhere else it prints the
//! same layout without colour, because `console` turns styling off when stdout is not a terminal.

use console::{measure_text_width, Style, Term};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn print(markdown: &str) {
    for line in render(markdown, width()) {
        println!("{line}");
    }
}

fn width() -> usize {
    let term = Term::stdout();
    if !term.is_term() {
        return 100;
    }
    usize::from(term.size().1).clamp(40, 100)
}

/// A run of text and the style it is drawn in.
type Piece = (String, Style);

/// One level of list nesting: the next number for an ordered list, and how wide its marker is,
/// which is how far its wrapped lines are indented.
struct List {
    next: Option<u64>,
    width: usize,
}

/// A table while its cells are still arriving. The first row is the header.
#[derive(Default)]
struct Table {
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    cell: String,
}

struct Renderer {
    width: usize,
    out: Vec<String>,
    /// Inline styles, innermost last.
    styles: Vec<Style>,
    /// The block being built.
    pieces: Vec<Piece>,
    quote: usize,
    lists: Vec<List>,
    /// The marker owed to the first block of the current list item.
    marker: Option<String>,
    code: bool,
    /// The link being built: where it points, and where its text started in `pieces`.
    link: Option<(String, usize)>,
    table: Option<Table>,
    /// A blank line is owed before the next block.
    gap: bool,
}

pub fn render(markdown: &str, width: usize) -> Vec<String> {
    let mut renderer = Renderer {
        width: width.max(20),
        out: Vec::new(),
        styles: vec![Style::new()],
        pieces: Vec::new(),
        quote: 0,
        lists: Vec::new(),
        marker: None,
        code: false,
        link: None,
        table: None,
        gap: false,
    };
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    for event in Parser::new_ext(markdown, options) {
        renderer.event(event);
    }
    renderer.flush();
    renderer.out
}

fn heading(base: Style, level: HeadingLevel) -> Style {
    match level {
        HeadingLevel::H1 | HeadingLevel::H2 => base.bold().cyan(),
        _ => base.bold(),
    }
}

impl Renderer {
    fn style(&self) -> Style {
        self.styles.last().cloned().unwrap_or_else(Style::new)
    }

    fn push_style(&mut self, change: impl FnOnce(Style) -> Style) {
        let next = change(self.style());
        self.styles.push(next);
    }

    fn pop_style(&mut self) {
        // The base style is never popped, however unbalanced the markdown is.
        if self.styles.len() > 1 {
            self.styles.pop();
        }
    }

    fn push(&mut self, text: &str, style: Style) {
        self.pieces.push((text.to_string(), style));
    }

    fn event(&mut self, event: Event) {
        if self.table.is_some() {
            return self.table_event(event);
        }
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) if self.code => self.code_lines(&text),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => self.push(&text, self.style()),
            Event::Code(text) | Event::InlineMath(text) | Event::DisplayMath(text) => self.push(&text, Style::new().yellow()),
            Event::FootnoteReference(name) => self.push(&format!("[^{name}]"), self.style()),
            Event::SoftBreak => self.push(" ", self.style()),
            // A hard break ends the line but not the block: the next line keeps the indent.
            Event::HardBreak => self.flush(),
            Event::Rule => self.rule(),
            Event::TaskListMarker(done) => self.push(if done { "[x] " } else { "[ ] " }, self.style()),
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => self.flush(),
            Tag::Heading { level, .. } => {
                self.flush();
                let style = heading(self.style(), level);
                self.styles.push(style);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.take_gap();
                self.code = true;
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push(List { next: start, width: 0 });
            }
            Tag::Item => {
                self.flush();
                self.marker = self.next_marker();
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(Table::default());
            }
            Tag::Emphasis => self.push_style(|s| s.italic()),
            Tag::Strong => self.push_style(|s| s.bold()),
            Tag::Strikethrough => self.push_style(|s| s.strikethrough()),
            Tag::Link { dest_url, .. } => {
                self.link = Some((dest_url.to_string(), self.pieces.len()));
                self.push_style(|s| s.underlined());
            }
            // Everything else still sends its text, which lands as ordinary text.
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush();
                self.gap = true;
            }
            TagEnd::Heading(_) => {
                self.flush();
                self.pop_style();
                self.gap = true;
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote = self.quote.saturating_sub(1);
                self.gap = true;
            }
            TagEnd::CodeBlock => {
                self.code = false;
                self.gap = true;
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
                self.gap = self.gap || self.lists.is_empty();
            }
            TagEnd::Item => self.flush(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => self.pop_style(),
            TagEnd::Link => self.end_link(),
            _ => {}
        }
    }

    /// A link shows its text, then its address dimmed, unless the text already is the address.
    fn end_link(&mut self) {
        self.pop_style();
        let Some((url, from)) = self.link.take() else { return };
        let shown: String = self.pieces[from.min(self.pieces.len())..].iter().map(|(t, _)| t.as_str()).collect();
        if url.is_empty() || shown == url {
            return;
        }
        self.push(&format!(" ({url})"), Style::new().dim());
    }

    fn next_marker(&mut self) -> Option<String> {
        let level = self.lists.last_mut()?;
        let marker = match level.next {
            Some(n) => {
                level.next = Some(n + 1);
                format!("{n}. ")
            }
            None => "• ".to_string(),
        };
        level.width = measure_text_width(&marker);
        Some(marker)
    }

    /// What goes in front of every line at this depth: quote bars, then list indents. The innermost
    /// list level is left out when its marker takes that space on the first line.
    fn margin(&self, with_marker: bool) -> (String, usize) {
        let levels = match with_marker {
            true => &self.lists[..self.lists.len().saturating_sub(1)],
            false => &self.lists[..],
        };
        let indent: usize = levels.iter().map(|l| l.width).sum();
        let bars: String = (0..self.quote).map(|_| Style::new().dim().apply_to("│ ").to_string()).collect();
        (format!("{bars}{}", " ".repeat(indent)), self.quote * 2 + indent)
    }

    fn take_gap(&mut self) {
        if !std::mem::take(&mut self.gap) || self.out.is_empty() {
            return;
        }
        self.out.push(String::new());
    }

    /// Wrap the block that has been building and add it.
    fn flush(&mut self) {
        if self.pieces.is_empty() {
            return;
        }
        self.take_gap();
        let pieces = std::mem::take(&mut self.pieces);
        let marker = self.marker.take();
        let (hang, hang_width) = self.margin(false);
        let (outer, _) = self.margin(true);
        let room = self.width.saturating_sub(hang_width).max(20);
        for (index, line) in wrap(&pieces, room).into_iter().enumerate() {
            let text: String = line.iter().map(|(t, s)| s.apply_to(t).to_string()).collect();
            let lead = match (&marker, index) {
                (Some(marker), 0) => format!("{outer}{}", Style::new().cyan().apply_to(marker)),
                _ => hang.clone(),
            };
            self.out.push(format!("{lead}{text}"));
        }
    }

    /// Code keeps its lines exactly, behind a dim gutter.
    fn code_lines(&mut self, code: &str) {
        let (margin, _) = self.margin(false);
        for line in code.strip_suffix('\n').unwrap_or(code).split('\n') {
            let gutter = Style::new().dim().apply_to("▎ ");
            self.out.push(format!("{margin}{gutter}{}", Style::new().yellow().apply_to(line)));
        }
    }

    fn rule(&mut self) {
        self.flush();
        self.take_gap();
        let (margin, used) = self.margin(false);
        let line = "─".repeat(self.width.saturating_sub(used));
        self.out.push(format!("{margin}{}", Style::new().dim().apply_to(line)));
        self.gap = true;
    }

    /// Inside a table only the cell text matters; inline styling is dropped, not printed raw.
    fn table_event(&mut self, event: Event) {
        let Some(table) = self.table.as_mut() else { return };
        match event {
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                let row = std::mem::take(&mut table.row);
                table.rows.push(row);
            }
            Event::End(TagEnd::TableCell) => {
                let cell = std::mem::take(&mut table.cell);
                table.row.push(cell.trim().to_string());
            }
            Event::End(TagEnd::Table) => self.finish_table(),
            Event::Text(t) | Event::Code(t) | Event::Html(t) | Event::InlineHtml(t) => table.cell.push_str(&t),
            Event::SoftBreak | Event::HardBreak => table.cell.push(' '),
            _ => {}
        }
    }

    fn finish_table(&mut self) {
        let Some(table) = self.table.take() else { return };
        self.take_gap();
        let (margin, used) = self.margin(false);
        for line in grid(&table.rows, self.width.saturating_sub(used)) {
            self.out.push(format!("{margin}{line}"));
        }
        self.gap = true;
    }
}

/// Split into runs of spaces and runs of everything else, so wrapping happens only between words.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut last: Option<bool> = None;
    for (index, c) in text.char_indices() {
        let space = c.is_whitespace();
        if last.is_some_and(|was| was != space) {
            out.push(&text[start..index]);
            start = index;
        }
        last = Some(space);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Styled pieces as lines no wider than `room`, breaking only between words.
fn wrap(pieces: &[Piece], room: usize) -> Vec<Vec<Piece>> {
    let mut lines: Vec<Vec<Piece>> = vec![Vec::new()];
    let mut used = 0;
    for (text, style) in pieces {
        for token in tokens(text) {
            let space = token.chars().all(char::is_whitespace);
            let cells = measure_text_width(token);
            if space && used == 0 {
                continue;
            }
            if !space && used > 0 && used + cells > room {
                trim_end(lines.last_mut());
                lines.push(Vec::new());
                used = 0;
            }
            push_token(&mut lines, token, style);
            used += cells;
        }
    }
    trim_end(lines.last_mut());
    lines
}

fn push_token(lines: &mut [Vec<Piece>], token: &str, style: &Style) {
    let Some(line) = lines.last_mut() else { return };
    line.push((token.to_string(), style.clone()));
}

fn trim_end(line: Option<&mut Vec<Piece>>) {
    let Some(line) = line else { return };
    while line.last().is_some_and(|(text, _)| text.trim().is_empty()) {
        line.pop();
    }
}

/// A table fitted to `width`: narrow columns keep their natural width, wide ones share the rest,
/// and a long cell wraps inside its own column.
fn grid(rows: &[Vec<String>], width: usize) -> Vec<String> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return Vec::new();
    }
    let natural: Vec<usize> = (0..columns)
        .map(|c| rows.iter().filter_map(|row| row.get(c)).map(|cell| measure_text_width(cell)).max().unwrap_or(0).max(1))
        .collect();
    let widths = fit(&natural, width.saturating_sub(3 * columns + 1));
    let border = Style::new().dim();
    let rule = |left: &str, join: &str, right: &str| {
        let segments: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        border.apply_to(format!("{left}{}{right}", segments.join(join))).to_string()
    };
    let mut out = vec![rule("┌", "┬", "┐")];
    for (index, row) in rows.iter().enumerate() {
        let style = if index == 0 { Style::new().bold() } else { Style::new() };
        out.extend(cells(row, &widths, &style, &border));
        if index == 0 {
            out.push(rule("├", "┼", "┤"));
        }
    }
    out.push(rule("└", "┴", "┘"));
    out
}

/// One table row as lines: every cell wrapped to its column, padded to the tallest.
fn cells(row: &[String], widths: &[usize], style: &Style, border: &Style) -> Vec<String> {
    let wrapped: Vec<Vec<String>> = widths
        .iter()
        .enumerate()
        .map(|(c, w)| {
            let text = row.get(c).cloned().unwrap_or_default();
            wrap(&[(text, Style::new())], *w).into_iter().map(|line| line.into_iter().map(|(t, _)| t).collect()).collect()
        })
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
    (0..height)
        .map(|line| {
            let parts: Vec<String> = widths
                .iter()
                .enumerate()
                .map(|(c, w)| {
                    let text = wrapped[c].get(line).cloned().unwrap_or_default();
                    let pad = w.saturating_sub(measure_text_width(&text));
                    format!("{}{}", style.apply_to(&text), " ".repeat(pad))
                })
                .collect();
            let bar = border.apply_to("│").to_string();
            format!("{bar} {} {bar}", parts.join(&format!(" {bar} ")))
        })
        .collect()
}

/// The widest cap that fits the columns into `room`, so narrow columns keep their natural width.
fn fit(natural: &[usize], room: usize) -> Vec<usize> {
    if natural.iter().sum::<usize>() <= room {
        return natural.to_vec();
    }
    let total = |cap: usize| natural.iter().map(|w| (*w).min(cap)).sum::<usize>();
    let mut cap = natural.iter().copied().max().unwrap_or(1);
    while cap > 1 && total(cap) > room {
        cap -= 1;
    }
    natural.iter().map(|w| (*w).min(cap).max(1)).collect()
}
