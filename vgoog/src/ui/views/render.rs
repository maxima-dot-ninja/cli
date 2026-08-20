use crate::ui::app::{App, Screen, Service};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

const BRAND_COLOR: Color = Color::Rgb(66, 133, 244);
const ACCENT_COLOR: Color = Color::Rgb(52, 168, 83);
const WARN_COLOR: Color = Color::Rgb(251, 188, 4);
const ERR_COLOR: Color = Color::Rgb(234, 67, 53);
const DIM_COLOR: Color = Color::Rgb(100, 100, 100);

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(1),   // main
            Constraint::Length(3), // status bar
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_status_bar(f, app, chunks[2]);

    match app.screen {
        Screen::ServiceSelect => render_service_select(f, app, chunks[1]),
        Screen::ActionSelect => render_action_select(f, app, chunks[1]),
        Screen::ActionView => render_action_view(f, app, chunks[1]),
        Screen::Input => render_input_form(f, app, chunks[1]),
        Screen::Confirm => {
            render_action_view(f, app, chunks[1]);
            render_confirm_dialog(f, app, chunks[1]);
        }
    }

    // Account switcher overlay (on top of everything)
    if app.show_account_switcher {
        render_account_switcher(f, app, f.area());
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let title = match &app.service {
        Some(svc) => format!("  vgoog  >  {} {}", svc.icon(), svc.name()),
        None => "  vgoog  —  Google Workspace Manager".to_string(),
    };

    let account_info = if app.account_label.is_empty() {
        String::new()
    } else {
        format!("👤 {}  ", app.account_label)
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(account_info.len() as u16 + 2)])
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(title, Style::default().fg(BRAND_COLOR).add_modifier(Modifier::BOLD)),
    ]))
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(DIM_COLOR)),
    );
    f.render_widget(header, chunks[0]);

    let account_widget = Paragraph::new(Line::from(vec![
        Span::styled(account_info, Style::default().fg(ACCENT_COLOR)),
    ]))
    .alignment(ratatui::layout::Alignment::Right)
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(DIM_COLOR)),
    );
    f.render_widget(account_widget, chunks[1]);
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let help = match app.screen {
        Screen::ServiceSelect => "↑↓ Navigate  ⏎ Select  ^A Account  q Quit",
        Screen::ActionSelect => "↑↓ Navigate  ⏎ Select  ^A Account  Esc Back  q Quit",
        Screen::ActionView => "↑↓ Navigate  ⏎ Detail  d Delete  n Next  ^A Account  Esc Back",
        Screen::Input => "Tab Next Field  ⏎ Submit  Esc Cancel",
        Screen::Confirm => "y Confirm  n Cancel",
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let status_color = if app.status_message.starts_with("Error") {
        ERR_COLOR
    } else if app.loading {
        WARN_COLOR
    } else {
        ACCENT_COLOR
    };

    let status = Paragraph::new(Span::styled(
        &app.status_message,
        Style::default().fg(status_color),
    ))
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(DIM_COLOR)),
    );

    let help_text = Paragraph::new(Span::styled(
        help,
        Style::default().fg(DIM_COLOR),
    ))
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(DIM_COLOR)),
    )
    .alignment(ratatui::layout::Alignment::Right);

    f.render_widget(status, chunks[0]);
    f.render_widget(help_text, chunks[1]);
}

fn render_service_select(f: &mut Frame, app: &App, area: Rect) {
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(1),
    };

    let items: Vec<ListItem> = Service::ALL
        .iter()
        .enumerate()
        .map(|(i, svc)| {
            let style = if i == app.selected_service {
                Style::default().fg(BRAND_COLOR).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if i == app.selected_service { "▸ " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{}  {}", svc.icon(), svc.name()), style),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn render_action_select(f: &mut Frame, app: &App, area: Rect) {
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(1),
    };

    let actions = app.current_actions();
    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let style = if i == app.selected_action {
                Style::default().fg(BRAND_COLOR).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if i == app.selected_action { "▸ " } else { "  " };
            ListItem::new(Span::styled(format!("{prefix}{action}"), style))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn render_action_view(f: &mut Frame, app: &App, area: Rect) {
    if app.items.is_empty() && app.detail.is_none() {
        let msg = if app.loading {
            "Loading..."
        } else {
            "No items found"
        };
        let inner = Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(1),
        };
        let p = Paragraph::new(Span::styled(msg, Style::default().fg(DIM_COLOR)));
        f.render_widget(p, inner);
        return;
    }

    if let Some(detail) = &app.detail {
        render_detail_view(f, detail, app.scroll_offset, area);
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // List panel — header line + items
    let list_header_area = Rect { height: 1, ..columns[0] };
    let list_body = Rect {
        x: columns[0].x + 1,
        y: columns[0].y + 1,
        width: columns[0].width.saturating_sub(1),
        height: columns[0].height.saturating_sub(1),
    };

    let page_info = if app.page_info.is_empty() {
        String::new()
    } else {
        format!(" ({})", app.page_info)
    };
    let header_line = Paragraph::new(Span::styled(
        format!(" Items{page_info}"),
        Style::default().fg(DIM_COLOR),
    ));
    f.render_widget(header_line, list_header_area);

    let items: Vec<ListItem> = app
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == app.item_cursor {
                Style::default().fg(BRAND_COLOR).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if i == app.item_cursor { "▸ " } else { "  " };
            ListItem::new(vec![
                Line::from(Span::styled(format!("{prefix}{}", item.title), style)),
                Line::from(Span::styled(
                    format!("  {}", item.subtitle),
                    Style::default().fg(DIM_COLOR),
                )),
            ])
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, list_body);

    // Preview panel — left border as divider, header line + content
    let preview_header_area = Rect { height: 1, ..columns[1] };
    let preview_body = Rect {
        x: columns[1].x + 1,
        y: columns[1].y + 1,
        width: columns[1].width.saturating_sub(1),
        height: columns[1].height.saturating_sub(1),
    };

    // Vertical divider
    let divider = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(DIM_COLOR));
    f.render_widget(divider, columns[1]);

    let preview_header = Paragraph::new(Span::styled(
        " Preview",
        Style::default().fg(DIM_COLOR),
    ));
    f.render_widget(preview_header, preview_header_area);

    if let Some(item) = app.items.get(app.item_cursor) {
        let preview_text = format_json_preview(&item.metadata);
        let preview = Paragraph::new(Text::from(preview_text))
            .wrap(Wrap { trim: false });
        f.render_widget(preview, preview_body);
    }
}

fn render_detail_view(f: &mut Frame, detail: &serde_json::Value, scroll: usize, area: Rect) {
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(1),
    };

    let text = serde_json::to_string_pretty(detail).unwrap_or_else(|_| format!("{detail:?}"));
    let lines: Vec<Line> = text
        .lines()
        .skip(scroll)
        .map(|l| {
            if l.contains('"') && l.contains(':') {
                let parts: Vec<&str> = l.splitn(2, ':').collect();
                Line::from(vec![
                    Span::styled(parts[0], Style::default().fg(BRAND_COLOR)),
                    Span::styled(":", Style::default().fg(DIM_COLOR)),
                    Span::styled(
                        parts.get(1).unwrap_or(&"").to_string(),
                        Style::default().fg(Color::White),
                    ),
                ])
            } else {
                Line::from(Span::styled(l, Style::default().fg(Color::White)))
            }
        })
        .collect();

    let detail_widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false });
    f.render_widget(detail_widget, inner);
}

fn render_input_form(f: &mut Frame, app: &App, area: Rect) {
    let inner = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(1),
    };

    let mut constraints: Vec<Constraint> = app
        .input_fields
        .iter()
        .map(|field| {
            if field.multiline {
                Constraint::Length(4) // label + 3 lines
            } else {
                Constraint::Length(2) // label + value
            }
        })
        .collect();
    constraints.push(Constraint::Min(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, field) in app.input_fields.iter().enumerate() {
        if i >= chunks.len().saturating_sub(1) {
            break;
        }
        let is_active = i == app.input_field_cursor;

        // Label line
        let label_style = if is_active {
            Style::default().fg(BRAND_COLOR).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM_COLOR)
        };
        let label = if field.required {
            format!("{}*", field.label)
        } else {
            field.label.clone()
        };
        let label_area = Rect { height: 1, ..chunks[i] };
        f.render_widget(
            Paragraph::new(Span::styled(label, label_style)),
            label_area,
        );

        // Value line(s)
        let value_area = Rect {
            y: chunks[i].y + 1,
            height: chunks[i].height.saturating_sub(1),
            ..chunks[i]
        };

        let value_style = if is_active {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Rgb(180, 180, 180))
        };

        let display_text = if field.value.is_empty() {
            Span::styled(&field.placeholder, Style::default().fg(DIM_COLOR))
        } else {
            Span::styled(&field.value, value_style)
        };

        let cursor_indicator = if is_active && field.value.is_empty() { "" } else if is_active { "█" } else { "" };
        let mut line_spans = vec![display_text];
        if is_active {
            line_spans.push(Span::styled(cursor_indicator, Style::default().fg(BRAND_COLOR)));
        }

        f.render_widget(
            Paragraph::new(Line::from(line_spans)),
            value_area,
        );
    }
}

fn render_confirm_dialog(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 30, area);
    f.render_widget(Clear, popup_area);
    let dialog = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            &app.confirm_message,
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [y] ", Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("Confirm   ", Style::default().fg(Color::White)),
            Span::styled(" [n] ", Style::default().fg(ERR_COLOR).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", Style::default().fg(Color::White)),
        ]),
    ])
    .block(
        Block::default()
            .title(" Confirm ")
            .title_style(Style::default().fg(WARN_COLOR).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(WARN_COLOR)),
    )
    .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(dialog, popup_area);
}

fn render_account_switcher(f: &mut Frame, app: &App, area: Rect) {
    let height = (app.account_list.len() as u16 + 4).min(area.height.saturating_sub(4));
    let width = 40u16.min(area.width.saturating_sub(4));
    let popup_area = Rect {
        x: area.width.saturating_sub(width + 2),
        y: 3,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = app
        .account_list
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let is_active = *name == app.account_name;
            let is_selected = i == app.account_cursor;
            let style = if is_selected {
                Style::default().fg(BRAND_COLOR).add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(ACCENT_COLOR)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { " ▸ " } else { "   " };
            let suffix = if is_active { " ●" } else { "" };
            ListItem::new(Span::styled(format!("{prefix}{name}{suffix}"), style))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Switch Account (^A) ")
            .title_style(Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(ACCENT_COLOR)),
    );
    f.render_widget(list, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn format_json_preview(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Object(map) => {
            let mut lines = Vec::new();
            for (k, v) in map.iter().take(15) {
                let v_str = match v {
                    serde_json::Value::String(s) => {
                        if s.len() > 60 {
                            format!("{}...", &s[..57])
                        } else {
                            s.clone()
                        }
                    }
                    serde_json::Value::Array(arr) => format!("[{} items]", arr.len()),
                    serde_json::Value::Object(_) => "{...}".to_string(),
                    other => other.to_string(),
                };
                lines.push(format!("{k}: {v_str}"));
            }
            lines.join("\n")
        }
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| format!("{other:?}")),
    }
}
