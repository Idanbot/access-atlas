use crate::app::{App, RefreshState, ThemeId};
use glam::DVec3;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget, Wrap},
};

const MASK_WIDTH: usize = 1440;
const MASK_HEIGHT: usize = 720;

pub const BRAILLE_DOTS: [(usize, usize, u8); 8] = [
    (0, 0, 0b0000_0001), // dot 1
    (0, 1, 0b0000_0010), // dot 2
    (0, 2, 0b0000_0100), // dot 3
    (1, 0, 0b0000_1000), // dot 4
    (1, 1, 0b0001_0000), // dot 5
    (1, 2, 0b0010_0000), // dot 6
    (0, 3, 0b0100_0000), // dot 7
    (1, 3, 0b1000_0000), // dot 8
];

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub name: &'static str,
    pub background: [u8; 3],
    pub ocean: [u8; 3],
    pub land: [u8; 3],
    pub coast: [u8; 3],
    pub border: [u8; 3],
    pub atmosphere: [u8; 3],
    pub active_target: [u8; 3],
    pub other_target: [u8; 3],
    pub origin: [u8; 3],
    pub route: [u8; 3],
    pub packet: [u8; 3],
    pub hud_accent: [u8; 3],
    pub hud_text: [u8; 3],
    pub border_color: [u8; 3],
}

pub fn get_theme(theme_id: ThemeId) -> ThemePalette {
    match theme_id {
        ThemeId::CyberOrbital => ThemePalette {
            name: "Orbital Ice",
            background: [3, 10, 16],
            ocean: [10, 42, 54],
            land: [28, 120, 134],
            coast: [132, 232, 239],
            border: [76, 164, 178],
            atmosphere: [55, 177, 204],
            active_target: [255, 190, 76],
            other_target: [76, 189, 209],
            origin: [180, 239, 244],
            route: [48, 196, 219],
            packet: [225, 252, 255],
            hud_accent: [146, 232, 241],
            hud_text: [125, 170, 183],
            border_color: [28, 91, 108],
        },
        ThemeId::TacticalRadar => ThemePalette {
            name: "Tactical Radar (P31)",
            background: [2, 10, 4],
            ocean: [6, 28, 12],
            land: [28, 115, 50],
            coast: [65, 225, 105],
            border: [100, 185, 125],
            atmosphere: [50, 155, 75],
            active_target: [220, 255, 225],
            other_target: [75, 185, 105],
            origin: [160, 255, 180],
            route: [130, 255, 155],
            packet: [245, 255, 245],
            hud_accent: [80, 240, 120],
            hud_text: [120, 200, 140],
            border_color: [30, 120, 55],
        },
        ThemeId::MinimalAtlas => ThemePalette {
            name: "Minimal Slate Atlas",
            background: [14, 18, 24],
            ocean: [22, 38, 48],
            land: [72, 118, 98],
            coast: [220, 200, 130],
            border: [190, 190, 185],
            atmosphere: [90, 130, 155],
            active_target: [255, 198, 75],
            other_target: [125, 175, 205],
            origin: [95, 165, 245],
            route: [240, 95, 80],
            packet: [255, 235, 210],
            hud_accent: [225, 205, 150],
            hud_text: [175, 185, 195],
            border_color: [60, 85, 105],
        },
        ThemeId::AmberCrt => ThemePalette {
            name: "Amber CRT",
            background: [16, 8, 2],
            ocean: [36, 18, 4],
            land: [120, 62, 16],
            coast: [225, 130, 32],
            border: [180, 100, 28],
            atmosphere: [155, 85, 22],
            active_target: [255, 225, 140],
            other_target: [185, 105, 30],
            origin: [255, 140, 40],
            route: [255, 165, 50],
            packet: [255, 245, 210],
            hud_accent: [255, 180, 60],
            hud_text: [210, 150, 80],
            border_color: [120, 65, 20],
        },
        ThemeId::DeepSpace => ThemePalette {
            name: "Deep Space Nebula",
            background: [6, 4, 14],
            ocean: [18, 18, 48],
            land: [64, 46, 115],
            coast: [145, 110, 235],
            border: [115, 85, 195],
            atmosphere: [120, 85, 220],
            active_target: [255, 60, 160],
            other_target: [80, 180, 255],
            origin: [0, 235, 255],
            route: [0, 220, 255],
            packet: [255, 255, 255],
            hud_accent: [190, 120, 255],
            hud_text: [180, 165, 225],
            border_color: [85, 55, 140],
        },
    }
}

pub fn render(frame: &mut Frame, app: &App) {
    let theme = get_theme(app.theme());
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(to_color(theme.background))),
        area,
    );
    if area.width == 0 || area.height == 0 {
        return;
    }

    let layout = deck_layout(area);
    render_header(frame, layout.header, app, &theme);

    let globe_block = deck_block("00 // ORBITAL VIEW", &theme);
    let globe_area = globe_block.inner(layout.globe);
    frame.render_widget(globe_block, layout.globe);
    frame.render_widget(GlobeWidget { app, theme }, globe_area);

    render_command_rail(frame, layout.rail, app, &theme);
    render_footer(frame, layout.footer, &theme);
    if app.command_library_open() {
        render_command_library(frame, area, app, &theme);
    }
}

#[derive(Debug, Clone, Copy)]
struct DeckLayout {
    header: Rect,
    globe: Rect,
    rail: Rect,
    footer: Rect,
}

fn deck_layout(area: Rect) -> DeckLayout {
    let compact = area.width < 100 || area.height < 30;
    let header_height = if compact { 2 } else { 3 };
    let footer_height = if compact { 2 } else { 3 };
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(0),
            Constraint::Length(footer_height),
        ])
        .split(area);

    let desired_rail = match area.width {
        0..=89 => 34,
        90..=109 => 38,
        110..=139 => 44,
        _ => 52,
    };
    let rail_width = desired_rail.min(vertical[1].width.saturating_sub(32));
    let gutter = u16::from(area.width >= 100);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(gutter),
            Constraint::Length(rail_width),
        ])
        .split(vertical[1]);

    DeckLayout {
        header: vertical[0],
        globe: horizontal[0],
        rail: horizontal[2],
        footer: vertical[2],
    }
}

fn deck_block(title: &str, theme: &ThemePalette) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Plain)
        .title(Line::from(vec![
            Span::styled(" ◇ ", Style::default().fg(to_color(theme.active_target))),
            Span::styled(
                title.to_owned(),
                Style::default()
                    .fg(to_color(theme.hud_accent))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .border_style(Style::default().fg(to_color(theme.border_color)))
        .style(Style::default().bg(to_color(theme.background)))
}

fn render_header(frame: &mut Frame, area: Rect, app: &App, theme: &ThemePalette) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let target_number = format!(
        "LOCK {:02}/{:02}",
        app.target_index() + 1,
        app.topology().targets.len()
    );
    let cycle_state = if app.is_paused() {
        "AUTO HOLD"
    } else {
        "AUTO LIVE"
    };
    let title = if area.width < 92 {
        " ACCESS ATLAS // ORBITAL CMD"
    } else {
        " ACCESS ATLAS // ORBITAL COMMAND DECK"
    };
    let right = format!("● {cycle_state}  ·  {target_number}");
    let first = spaced_line(title, &right, area.width as usize);

    let second = if area.width < 96 {
        format!(
            " MISSION {} // {}/{} // RTT {:.0}MS // {}",
            app.topology().name,
            app.target().location.city,
            app.target().location.country,
            app.target().status.latency_ms,
            compact_palette_name(theme.name).to_uppercase()
        )
    } else {
        let mission = format!(
            " MISSION {}  ·  {} / {}  ·  RTT {:.0}ms",
            app.topology().name,
            app.target().location.city,
            app.target().location.country,
            app.target().status.latency_ms
        );
        let palette = format!("PALETTE {} ", theme.name.to_uppercase());
        spaced_line(&mission, &palette, area.width as usize)
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            first,
            Style::default()
                .fg(to_color(theme.hud_accent))
                .add_modifier(Modifier::BOLD),
        )]),
        Line::styled(second, Style::default().fg(to_color(theme.hud_text))),
    ];
    if area.height > 2 {
        lines.push(Line::styled(
            "━".repeat(area.width as usize),
            Style::default().fg(to_color(theme.border_color)),
        ));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(scale_color_as_color(theme.background, theme.ocean))),
        area,
    );
}

fn render_command_rail(frame: &mut Frame, area: Rect, app: &App, theme: &ThemePalette) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let status_height = if area.height <= 24 { 7 } else { 8 }.min(area.height);
    let remaining = area.height.saturating_sub(status_height);
    let access_height = match area.height {
        0..=20 => 7,
        21..=29 => 9,
        _ => 11,
    }
    .min(remaining);
    let panels = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(status_height),
            Constraint::Length(access_height),
            Constraint::Min(0),
        ])
        .split(area);

    render_target_status(frame, panels[0], app, theme);
    render_access_vector(frame, panels[1], app, theme);
    render_inspection(frame, panels[2], app, theme);
}

fn render_target_status(frame: &mut Frame, area: Rect, app: &App, theme: &ThemePalette) {
    let target = app.target();
    let health_style = if target.status.state.eq_ignore_ascii_case("healthy") {
        Style::default().fg(to_color(theme.hud_accent))
    } else {
        Style::default()
            .fg(to_color(theme.active_target))
            .add_modifier(Modifier::BOLD)
    };
    let kind = target.kind.replace('_', " ").to_uppercase();
    let latitude = if target.location.latitude >= 0.0 {
        'N'
    } else {
        'S'
    };
    let longitude = if target.location.longitude >= 0.0 {
        'E'
    } else {
        'W'
    };
    let metrics = if area.width < 40 {
        format!(
            "{:.0}ms RTT  ·  {:.1}%  ·  UP {}d",
            target.status.latency_ms,
            target.status.packet_loss_percent,
            target.status.uptime_seconds / 86_400
        )
    } else {
        format!(
            "{:.0}ms RTT  ·  {:.1}% LOSS  ·  {}",
            target.status.latency_ms,
            target.status.packet_loss_percent,
            format_uptime_short(target.status.uptime_seconds)
        )
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled("⌖ ", Style::default().fg(to_color(theme.active_target))),
            Span::styled(
                target.label.as_str(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("● ", health_style),
            Span::styled(target.status.state.to_uppercase(), health_style),
            Span::styled(
                format!(
                    "  ·  {} / {}",
                    target.location.city, target.location.country
                ),
                Style::default().fg(to_color(theme.hud_text)),
            ),
        ]),
        Line::styled(
            format!("{}  ·  {kind}", target.provider.to_uppercase()),
            Style::default().fg(to_color(theme.hud_text)),
        ),
        Line::styled(
            format!(
                "{:.2}°{latitude}  {:.2}°{longitude}",
                target.location.latitude.abs(),
                target.location.longitude.abs()
            ),
            Style::default().fg(Color::DarkGray),
        ),
        Line::styled(metrics, Style::default().fg(to_color(theme.hud_text))),
    ];
    if area.height >= 8 && app.current_connection().is_some() {
        let sync = if let Some(source) = app.current_source_report() {
            format!(
                "{:?} · {} found · {}",
                source.state, source.connections, source.message
            )
        } else {
            match app.refresh_state() {
                RefreshState::Running => "RUNNING · provider scan queued".to_owned(),
                _ => "CACHE · press R for online refresh".to_owned(),
            }
        };
        lines.push(Line::from(vec![
            Span::styled("SYNC   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                ellipsized_panel_text(&sync, area.width.saturating_sub(10) as usize),
                Style::default().fg(to_color(theme.hud_text)),
            ),
        ]));
    } else if area.height >= 8 {
        lines.push(Line::from(vec![
            Span::styled("CHECK  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                target.status.checked_at.as_str(),
                Style::default().fg(to_color(theme.hud_text)),
            ),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines).block(deck_block("01 // TARGET LOCK", theme)),
        area,
    );
}

fn render_access_vector(frame: &mut Frame, area: Rect, app: &App, theme: &ThemePalette) {
    let target = app.target();
    let network = app.current_network_type();
    let option = app.current_access_option();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("NETWORK ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{:02}/{:02}  {}",
                    app.network_type_index() + 1,
                    target.network_types.len(),
                    network.label.to_uppercase()
                ),
                Style::default()
                    .fg(to_color(theme.hud_accent))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("$ ", Style::default().fg(to_color(theme.active_target))),
            Span::styled(
                network.binary.as_str(),
                Style::default().fg(to_color(theme.hud_accent)),
            ),
        ]),
        Line::from(vec![
            Span::styled("OPTION  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "{:02}/{:02}  {}",
                    app.access_option_index() + 1,
                    network.access_options.len(),
                    option.label
                ),
                Style::default()
                    .fg(to_color(theme.active_target))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let expanded = area.height >= 11;
    let command = wrap_text_for_panel(
        &option.command,
        area.width.saturating_sub(4) as usize,
        if expanded { 2 } else { 1 },
    );
    for (index, chunk) in command.into_iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled(
                if index == 0 { "› " } else { "  " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(chunk, Style::default().fg(Color::White)),
        ]));
    }

    let route = wrap_text_for_panel(
        &option.route.join(" → "),
        area.width.saturating_sub(10) as usize,
        if expanded { 2 } else { 1 },
    );
    for (index, chunk) in route.into_iter().enumerate() {
        lines.push(Line::from(vec![
            Span::styled(
                if index == 0 { "ROUTE   " } else { "        " },
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(chunk, Style::default().fg(to_color(theme.hud_text))),
        ]));
    }
    if !app.extended_commands().is_empty() {
        let refresh = match app.refresh_state() {
            RefreshState::Idle => "R REFRESH".to_owned(),
            RefreshState::Running => "REFRESHING…".to_owned(),
            RefreshState::Complete { loaded, failed } => {
                format!("R REFRESH {loaded} OK/{failed} ERR")
            }
            RefreshState::Failed(_) => "R REFRESH FAILED".to_owned(),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" ENTER {} COMMANDS ", app.extended_commands().len()),
                Style::default()
                    .fg(to_color(theme.background))
                    .bg(to_color(theme.active_target))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  Y COPY  ",
                Style::default().fg(to_color(theme.hud_accent)),
            ),
            Span::styled(refresh, Style::default().fg(to_color(theme.hud_text))),
        ]));
    }
    if area.height >= 9 && app.extended_commands().is_empty() {
        let description =
            ellipsized_panel_text(&network.description, area.width.saturating_sub(10) as usize);
        lines.push(Line::from(vec![
            Span::styled("ABOUT   ", Style::default().fg(Color::DarkGray)),
            Span::styled(description, Style::default().fg(to_color(theme.hud_text))),
        ]));
    }
    if area.height >= 11 {
        let notes = ellipsized_panel_text(&option.notes, area.width.saturating_sub(10) as usize);
        lines.push(Line::from(vec![
            Span::styled("NOTE    ", Style::default().fg(Color::DarkGray)),
            Span::styled(notes, Style::default().fg(to_color(theme.hud_text))),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines).block(deck_block("02 // ACCESS VECTOR [RO]", theme)),
        area,
    );
}

fn render_command_library(frame: &mut Frame, area: Rect, app: &App, theme: &ThemePalette) {
    let popup = centered_rect(area, 86, 76, 62, 18);
    frame.render_widget(Clear, popup);
    let commands = app.extended_commands();
    let selected = app.command_library_index();
    let mut lines = vec![Line::from(vec![
        Span::styled("FILTER  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            if app.command_search_active() {
                format!("/{}▌", app.command_filter())
            } else if app.command_filter().is_empty() {
                "/ to search".to_owned()
            } else {
                format!("/{}", app.command_filter())
            },
            Style::default().fg(to_color(theme.hud_accent)),
        ),
    ])];
    lines.extend(
        app.visible_extended_commands()
            .into_iter()
            .map(|(index, command)| {
                let active = index == selected;
                let style = if active {
                    Style::default()
                        .fg(to_color(theme.background))
                        .bg(to_color(theme.active_target))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(to_color(theme.hud_text))
                };
                Line::styled(
                    format!(
                        " {} {:02}/{:02}  {:<12}  {} ",
                        if active { "›" } else { " " },
                        index + 1,
                        commands.len(),
                        format!("{:?}", command.kind).to_uppercase(),
                        command.label
                    ),
                    style,
                )
            }),
    );
    if let Some(command) = commands.get(selected) {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("$ ", Style::default().fg(to_color(theme.active_target))),
            Span::styled(command.command.as_str(), Style::default().fg(Color::White)),
        ]));
        lines.push(Line::styled(
            command.description.as_str(),
            Style::default().fg(to_color(theme.hud_text)),
        ));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        key_span("↑/↓", theme),
        hint_span(" select  "),
        Span::styled(
            " Y COPY ",
            Style::default()
                .fg(to_color(theme.background))
                .bg(to_color(theme.hud_text))
                .add_modifier(Modifier::BOLD),
        ),
        hint_span("  "),
        key_span("Esc", theme),
        hint_span(" close"),
    ]));
    let block = deck_block("04 // COMMAND LIBRARY [READ ONLY]", theme)
        .style(Style::default().bg(to_color(theme.background)));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn centered_rect(
    area: Rect,
    width_percent: u16,
    height_percent: u16,
    min_width: u16,
    min_height: u16,
) -> Rect {
    let width = ((u32::from(area.width) * u32::from(width_percent) / 100) as u16)
        .max(min_width.min(area.width))
        .min(area.width);
    let height = ((u32::from(area.height) * u32::from(height_percent) / 100) as u16)
        .max(min_height.min(area.height))
        .min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn render_inspection(frame: &mut Frame, area: Rect, app: &App, theme: &ThemePalette) {
    let details = app.detail_rows();
    let visible_rows = area.height.saturating_sub(2) as usize;
    let selected = app.detail_index().min(details.len().saturating_sub(1));
    let start = selected
        .saturating_sub(visible_rows.saturating_sub(1) / 2)
        .min(details.len().saturating_sub(visible_rows));
    let label_width: usize = match area.width {
        0..=35 => 11,
        36..=46 => 14,
        _ => 17,
    };
    let value_width = area.width.saturating_sub(label_width as u16 + 5) as usize;
    let lines = details
        .iter()
        .enumerate()
        .skip(start)
        .take(visible_rows)
        .map(|(index, row)| {
            let is_selected = index == selected;
            let marker = if is_selected { "›" } else { " " };
            let label = compact_detail_label(&row.label, label_width);
            let value = ellipsized_panel_text(&row.value, value_width);
            let value_style = if is_selected {
                Style::default()
                    .fg(to_color(theme.active_target))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(to_color(theme.hud_text))
            };
            Line::from(vec![
                Span::styled(
                    format!("{marker} {label:<label_width$} "),
                    if is_selected {
                        Style::default().fg(to_color(theme.active_target))
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(value, value_style),
            ])
        })
        .collect::<Vec<_>>();
    let title = format!("03 // INSPECTION  {:02}/{:02}", selected + 1, details.len());
    frame.render_widget(Paragraph::new(lines).block(deck_block(&title, theme)), area);
}

fn render_footer(frame: &mut Frame, area: Rect, theme: &ThemePalette) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut lines = Vec::with_capacity(3);
    if area.height > 2 {
        lines.push(Line::styled(
            "━".repeat(area.width as usize),
            Style::default().fg(to_color(theme.border_color)),
        ));
    }
    lines.push(Line::from(vec![
        key_span("←/→", theme),
        hint_span(" target  "),
        key_span("Tab", theme),
        hint_span(" access  "),
        key_span("↑/↓", theme),
        hint_span(" inspect  "),
        key_span("Space", theme),
        hint_span(" auto  "),
        key_span("q", theme),
        hint_span(" quit"),
    ]));
    if area.height > 1 {
        lines.push(Line::from(vec![
            key_span("h j k l", theme),
            hint_span(" orbit  "),
            key_span("+ / -", theme),
            hint_span(" zoom  "),
            key_span("r", theme),
            hint_span(" recenter  "),
            key_span("t", theme),
            hint_span(" palette"),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(to_color(theme.background))),
        area,
    );
}

fn key_span(label: &'static str, theme: &ThemePalette) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(to_color(theme.background))
            .bg(to_color(theme.hud_text))
            .add_modifier(Modifier::BOLD),
    )
}

fn hint_span(label: &'static str) -> Span<'static> {
    Span::styled(label, Style::default().fg(Color::DarkGray))
}

fn spaced_line(left: &str, right: &str, width: usize) -> String {
    let used = left.chars().count() + right.chars().count();
    format!("{left}{}{right}", " ".repeat(width.saturating_sub(used)))
}

fn compact_detail_label(label: &str, width: usize) -> String {
    let compact = label
        .strip_prefix("metadata.")
        .or_else(|| label.strip_prefix("network."))
        .or_else(|| label.strip_prefix("access."))
        .unwrap_or(label);
    if compact.chars().count() <= width {
        compact.to_owned()
    } else {
        compact
            .chars()
            .take(width.saturating_sub(1))
            .chain(['…'])
            .collect()
    }
}

fn compact_palette_name(name: &'static str) -> &'static str {
    match name {
        "Tactical Radar (P31)" => "Radar P31",
        "Minimal Slate Atlas" => "Slate Atlas",
        "Deep Space Nebula" => "Deep Space",
        other => other,
    }
}

fn wrap_text_for_panel(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    if width == 0 || max_lines == 0 {
        return Vec::new();
    }
    let mut lines = Vec::with_capacity(max_lines);
    let mut remaining = text.trim();
    for line_index in 0..max_lines {
        if remaining.is_empty() {
            break;
        }
        if remaining.chars().count() <= width {
            lines.push(remaining.to_owned());
            break;
        }
        if line_index + 1 == max_lines {
            let mut line = remaining
                .chars()
                .take(width.saturating_sub(1))
                .collect::<String>();
            line.push('…');
            lines.push(line);
            break;
        }

        let hard_end = remaining
            .char_indices()
            .nth(width)
            .map_or(remaining.len(), |(index, _)| index);
        let prefix = &remaining[..hard_end];
        let split = prefix
            .char_indices()
            .rev()
            .find(|(index, character)| *index > 0 && character.is_whitespace())
            .map_or(hard_end, |(index, _)| index);
        lines.push(remaining[..split].trim_end().to_owned());
        remaining = remaining[split..].trim_start();
    }
    lines
}

fn ellipsized_panel_text(text: &str, width: usize) -> String {
    wrap_text_for_panel(text, width, 1)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn format_uptime_short(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    format!("{days}d {hours:02}h UP")
}

fn scale_color_as_color(background: [u8; 3], tint: [u8; 3]) -> Color {
    Color::Rgb(
        ((u16::from(background[0]) * 3 + u16::from(tint[0])) / 4) as u8,
        ((u16::from(background[1]) * 3 + u16::from(tint[1])) / 4) as u8,
        ((u16::from(background[2]) * 3 + u16::from(tint[2])) / 4) as u8,
    )
}

struct GlobeWidget<'a> {
    app: &'a App,
    theme: ThemePalette,
}

impl Widget for GlobeWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_globe(area, buf, self.app, &self.theme);
    }
}

#[derive(Clone, Copy)]
struct PointMarker {
    x: f64,
    y: f64,
    radius_sq: f64,
    color: [u8; 3],
    priority: u8,
}

#[derive(Clone, Copy)]
struct RingMarker {
    x: f64,
    y: f64,
    radius: f64,
    color: [u8; 3],
    priority: u8,
}

#[derive(Clone, Copy)]
struct RouteSegment {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color: [u8; 3],
    priority: u8,
}

#[derive(Clone, Copy)]
struct GlobeGeometry {
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    pitch: f64,
    rotation_cos: f64,
    rotation_sin: f64,
    pitch_cos: f64,
    pitch_sin: f64,
}

#[derive(Clone, Copy)]
struct DotSample {
    color: [u8; 3],
    priority: u8,
}

fn render_globe(area: Rect, buf: &mut Buffer, app: &App, theme: &ThemePalette) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let pixel_width = area.width as usize * 2;
    let pixel_height = area.height as usize * 4;
    let geometry = GlobeGeometry {
        center_x: pixel_width as f64 * 0.46,
        center_y: pixel_height as f64 * 0.51,
        radius_x: (pixel_width as f64 * 0.365 * app.zoom()).max(1.0),
        radius_y: (pixel_height as f64 * 0.365 * app.zoom()).max(1.0),
        rotation: app.rotation(),
        pitch: app.pitch(),
        rotation_cos: app.rotation().cos(),
        rotation_sin: app.rotation().sin(),
        pitch_cos: app.pitch().cos(),
        pitch_sin: app.pitch().sin(),
    };
    let (points, rings, segments) = build_overlays(app, &geometry, theme);
    let overlay = rasterize_overlays(pixel_width, pixel_height, &points, &rings, &segments);

    for cell_y in 0..area.height as usize {
        for cell_x in 0..area.width as usize {
            let mut bits = 0_u8;
            let mut best_sample = DotSample {
                color: theme.background,
                priority: 0,
            };
            // Full 8-dot Braille grid (2x4 subpixels) for high resolution
            for (dot_x, dot_y, mask) in BRAILLE_DOTS {
                let x = cell_x * 2 + dot_x;
                let y = cell_y * 4 + dot_y;
                if let Some(sample) = dot_sample(x, y, pixel_width, &geometry, &overlay, theme) {
                    bits |= mask;
                    if sample.priority >= best_sample.priority {
                        best_sample = sample;
                    }
                }
            }
            let glyph = braille_glyph(bits);
            let style = if bits == 0 {
                Style::default().bg(to_color(theme.background))
            } else {
                Style::default()
                    .fg(to_color(best_sample.color))
                    .bg(to_color(theme.background))
            };
            let cell = &mut buf[(area.x + cell_x as u16, area.y + cell_y as u16)];
            cell.set_char(glyph);
            cell.set_style(style);
        }
    }

    render_city_label(area, buf, app, &geometry, theme);
    render_telemetry_hud(area, buf, app, theme);
}

fn render_city_label(
    area: Rect,
    buf: &mut Buffer,
    app: &App,
    geometry: &GlobeGeometry,
    theme: &ThemePalette,
) {
    if area.width < 24 || area.height < 9 {
        return;
    }
    let target = app.target();
    let point = geo_to_vec(target.location.latitude, target.location.longitude);
    let Some((x, y, _depth)) = project_vec_camera(
        point,
        geometry.rotation,
        geometry.pitch,
        geometry.center_x,
        geometry.center_y,
        geometry.radius_x,
        geometry.radius_y,
    ) else {
        return;
    };

    let label_title = format!("⌖ ── {}", target.label);
    let label_meta = format!(
        "   {} / {} · {}",
        target.provider.to_uppercase(),
        target.location.country,
        target.location.region
    );
    let point_x = area
        .x
        .saturating_add((x / 2.0).round().clamp(0.0, area.width as f64 - 1.0) as u16);
    let point_y = area
        .y
        .saturating_add((y / 4.0).round().clamp(0.0, area.height as f64 - 1.0) as u16);
    let title_width = label_title.chars().count().min(area.width as usize) as u16;
    let right_start = point_x.saturating_add(2);
    let draw_x = if right_start.saturating_add(title_width) < area.right() {
        right_start
    } else {
        point_x
            .saturating_sub(title_width.saturating_add(2))
            .max(area.x)
    };
    let draw_y = point_y.clamp(area.y.saturating_add(3), area.bottom().saturating_sub(3));

    draw_clipped(
        buf,
        area,
        draw_x,
        draw_y,
        &label_title,
        Style::default()
            .fg(to_color(theme.active_target))
            .bg(to_color(theme.background))
            .add_modifier(Modifier::BOLD),
    );
    if area.width >= 64 && draw_y.saturating_add(1) < area.bottom() {
        draw_clipped(
            buf,
            area,
            draw_x,
            draw_y + 1,
            &label_meta,
            Style::default()
                .fg(to_color(theme.hud_text))
                .bg(to_color(theme.background)),
        );
    }
}

fn render_telemetry_hud(area: Rect, buf: &mut Buffer, app: &App, theme: &ThemePalette) {
    if area.width < 28 || area.height < 5 {
        return;
    }

    let lat = app.camera_pitch_deg();
    let lat_dir = if lat >= 0.0 { "N" } else { "S" };
    let lon = app.camera_azimuth_deg();
    let lon_dir = if lon <= 180.0 { "E" } else { "W" };
    let lon_val = if lon <= 180.0 { lon } else { 360.0 - lon };

    let top_left = if area.width < 62 {
        format!(
            "AZ {:05.1}°{} · EL {:04.1}°{} · {:.2}×",
            lon_val,
            lon_dir,
            lat.abs(),
            lat_dir,
            app.zoom()
        )
    } else {
        format!(
            "CAM // AZ {:05.1}°{} · EL {:04.1}°{} · ZOOM {:.2}×",
            lon_val,
            lon_dir,
            lat.abs(),
            lat_dir,
            app.zoom()
        )
    };
    draw_clipped(
        buf,
        area,
        area.x + 1,
        area.y + 1,
        &top_left,
        Style::default()
            .fg(to_color(theme.hud_accent))
            .bg(to_color(theme.background)),
    );

    let top_right = format!("NORTH // {}", theme.name.to_uppercase());
    let top_right_len = top_right.chars().count() as u16;
    if area.width > top_left.chars().count() as u16 + top_right_len + 5 {
        draw_clipped(
            buf,
            area,
            area.right().saturating_sub(top_right_len + 1),
            area.y + 1,
            &top_right,
            Style::default()
                .fg(to_color(theme.hud_text))
                .bg(to_color(theme.background)),
        );
    }

    let filled = (app.route_progress() * 8.0).round() as usize;
    let route_meter = format!(
        "UPLINK [{}{}] {:03}%",
        "■".repeat(filled.min(8)),
        "·".repeat(8_usize.saturating_sub(filled)),
        (app.route_progress() * 100.0).round() as u8
    );
    let bot_right = if app.is_paused() {
        "AUTO // HOLD".to_owned()
    } else {
        format!(
            "AUTO // LIVE {:03.1}s",
            (6.0 - app.elapsed().as_secs_f64()).max(0.0)
        )
    };
    let bot_right_len = bot_right.chars().count() as u16;
    let bottom_y = area.bottom().saturating_sub(2);
    draw_clipped(
        buf,
        area,
        area.x + 1,
        bottom_y,
        &route_meter,
        Style::default()
            .fg(to_color(theme.route))
            .bg(to_color(theme.background)),
    );
    if area.width > route_meter.chars().count() as u16 + bot_right_len + 4 {
        draw_clipped(
            buf,
            area,
            area.right().saturating_sub(bot_right_len + 1),
            bottom_y,
            &bot_right,
            Style::default()
                .fg(if app.is_paused() {
                    to_color(theme.hud_text)
                } else {
                    to_color(theme.hud_accent)
                })
                .bg(to_color(theme.background)),
        );
    }
}

fn draw_clipped(buf: &mut Buffer, area: Rect, x: u16, y: u16, content: &str, style: Style) {
    if x < area.x || x >= area.right() || y < area.y || y >= area.bottom() {
        return;
    }
    buf.set_stringn(
        x,
        y,
        content,
        area.right().saturating_sub(x) as usize,
        style,
    );
}

fn build_overlays(
    app: &App,
    geometry: &GlobeGeometry,
    theme: &ThemePalette,
) -> (Vec<PointMarker>, Vec<RingMarker>, Vec<RouteSegment>) {
    let mut points = Vec::with_capacity(32);
    let mut rings = Vec::with_capacity(8);
    let mut segments = Vec::with_capacity(64);

    let target = app.target();
    let origin = geo_to_vec(
        app.topology().origin.location.latitude,
        app.topology().origin.location.longitude,
    );

    // 1. Workstation Origin Marker (Clean compact beacon)
    if let Some((ox, oy, _depth)) = project_vec_camera(
        origin,
        geometry.rotation,
        geometry.pitch,
        geometry.center_x,
        geometry.center_y,
        geometry.radius_x,
        geometry.radius_y,
    ) {
        points.push(PointMarker {
            x: ox,
            y: oy,
            radius_sq: 1.25,
            color: theme.origin,
            priority: 4,
        });
    }

    // 2. Elevated great-circle uplink. During acquisition, the photon leads the
    // reveal; after lock, it loops at a restrained ambient cadence. The sample
    // budget follows the actual angular distance so the projected line stays
    // faithful on both short local hops and long intercontinental arcs.
    if app.route_progress() > 0.0 {
        let destination = geo_to_vec(target.location.latitude, target.location.longitude);
        let arc_angle = origin.dot(destination).clamp(-1.0, 1.0).acos();
        let steps = route_sample_count(arc_angle, geometry.radius_x, geometry.radius_y);
        let visible_steps = (steps as f32 * app.route_progress()).ceil() as usize;
        let packet_step = if app.route_progress() < 0.995 {
            visible_steps.min(steps)
        } else {
            let packet_phase = (app.continuous_time().as_secs_f64() * 0.24).fract();
            (packet_phase * steps as f64).round() as usize
        };

        let mut prev_pt: Option<(f64, f64)> = None;

        for step in 0..=visible_steps.min(steps) {
            let amount = step as f64 / steps as f64;
            let sphere_point = interpolate_arc_with_angle(origin, destination, amount, arc_angle);
            let altitude = (amount * std::f64::consts::PI).sin() * 0.16;
            let elevated_point = sphere_point * (1.0 + altitude);

            if let Some((x, y, _depth)) = project_vec_camera(
                elevated_point,
                geometry.rotation,
                geometry.pitch,
                geometry.center_x,
                geometry.center_y,
                geometry.radius_x,
                geometry.radius_y,
            ) {
                if let Some((px, py)) = prev_pt {
                    let (color, priority) = if step == packet_step {
                        (theme.packet, 5)
                    } else if step < packet_step && packet_step - step <= 4 {
                        let tail_fade = 1.0 - (packet_step - step) as f64 / 4.0;
                        (scale_color(theme.packet, tail_fade * 0.75 + 0.16), 3)
                    } else {
                        (scale_color(theme.route, 0.76), 2)
                    };

                    segments.push(RouteSegment {
                        x1: px,
                        y1: py,
                        x2: x,
                        y2: y,
                        color,
                        priority,
                    });
                }

                // Traveling photon packet head dot
                if step == packet_step {
                    points.push(PointMarker {
                        x,
                        y,
                        radius_sq: 1.25,
                        color: theme.packet,
                        priority: 5,
                    });
                }

                prev_pt = Some((x, y));
            } else {
                prev_pt = None;
            }
        }
    }

    // 3. Targets (Active and Inactive targets)
    for (index, target_item) in app.topology().targets.iter().enumerate() {
        let point = geo_to_vec(
            target_item.location.latitude,
            target_item.location.longitude,
        );
        if let Some((x, y, _depth)) = project_vec_camera(
            point,
            geometry.rotation,
            geometry.pitch,
            geometry.center_x,
            geometry.center_y,
            geometry.radius_x,
            geometry.radius_y,
        ) {
            let active = index == app.target_index();
            if active {
                // A small, crisp center keeps the lock readable without
                // obscuring the underlying map detail.
                points.push(PointMarker {
                    x,
                    y,
                    radius_sq: 0.64,
                    color: theme.active_target,
                    priority: 4,
                });

                // One restrained pulse gives the selected target a beacon-like
                // cadence while keeping the reticle compact at terminal scale.
                let t = app.continuous_time().as_secs_f64();
                let phase = (t * 0.72).fract();
                let radius = 1.15 + phase * 2.35;
                let fade = smoother_fade(phase) * 0.72;
                if fade > 0.04 {
                    rings.push(RingMarker {
                        x,
                        y,
                        radius,
                        color: scale_color(theme.active_target, fade),
                        priority: 3,
                    });
                }
            } else {
                points.push(PointMarker {
                    x,
                    y,
                    radius_sq: 0.8,
                    color: theme.other_target,
                    priority: 2,
                });
            }
        }
    }

    (points, rings, segments)
}

fn dot_sample(
    x: usize,
    y: usize,
    pixel_width: usize,
    geometry: &GlobeGeometry,
    overlay: &[Option<DotSample>],
    theme: &ThemePalette,
) -> Option<DotSample> {
    let screen_x = x as f64 + 0.5;
    let screen_y = y as f64 + 0.5;

    let best_overlay = overlay[y * pixel_width + x];

    if let Some(top) = best_overlay
        && top.priority >= 3
    {
        return Some(top);
    }

    let normalized_x = (screen_x - geometry.center_x) / geometry.radius_x;
    let normalized_y = (screen_y - geometry.center_y) / geometry.radius_y;
    let sphere_distance = normalized_x * normalized_x + normalized_y * normalized_y;

    // Inside globe sphere:
    if sphere_distance <= 1.0 {
        let x_double_prime = normalized_x;
        let y_double_prime = -normalized_y;
        let z_double_prime = (1.0 - sphere_distance).sqrt();

        // Invert pitch around X-axis
        let y_prime = y_double_prime * geometry.pitch_cos + z_double_prime * geometry.pitch_sin;
        let z_prime = -y_double_prime * geometry.pitch_sin + z_double_prime * geometry.pitch_cos;
        let x_prime = x_double_prime;

        // Invert yaw around Y-axis
        let x = x_prime * geometry.rotation_cos + z_prime * geometry.rotation_sin;
        let z = -x_prime * geometry.rotation_sin + z_prime * geometry.rotation_cos;
        let y = y_prime;

        let world_normal = DVec3::new(x, y, z);
        let latitude = y.asin().to_degrees();
        let longitude = x.atan2(z).to_degrees();

        let brightness = (-world_normal.x * 0.35 + world_normal.y * 0.45 + world_normal.z * 0.82)
            .max(0.0)
            .mul_add(0.55, 0.45)
            .min(1.0);

        let map = map_sample(latitude, longitude);

        // Solid continent landmasses, glowing coastlines, and crisp country borders
        if map.land || map.coast || map.boundary {
            let base_color = if map.boundary {
                theme.border
            } else if map.coast {
                theme.coast
            } else {
                theme.land
            };

            let color = scale_color(base_color, brightness);
            let sample = DotSample { color, priority: 1 };
            if let Some(top) = best_overlay
                && top.priority >= sample.priority
            {
                return Some(top);
            }
            return Some(sample);
        }

        if let Some(top) = best_overlay {
            return Some(top);
        }

        let graticule =
            distance_to_grid(latitude, 15.0) < 0.16 || distance_to_grid(longitude, 15.0) < 0.16;
        let density = if graticule { 0.58 } else { 0.055 };
        if world_stipple(latitude, longitude) < density {
            let ocean_color = if graticule {
                scale_color(theme.border, brightness * 0.34)
            } else {
                scale_color(theme.ocean, brightness * 0.82)
            };
            return Some(DotSample {
                color: ocean_color,
                priority: 1,
            });
        }

        return None;
    }

    // Atmospheric limb glow (1.0 < sphere_distance <= 1.05)
    if sphere_distance <= 1.05 {
        let limb_intensity = (1.05 - sphere_distance) / 0.05;
        let halo_stipple = ((x.wrapping_mul(41) + y.wrapping_mul(19)) % 100) as f64 / 100.0;
        if halo_stipple < limb_intensity * 0.55 {
            return Some(DotSample {
                color: scale_color(theme.atmosphere, limb_intensity * 0.85),
                priority: 1,
            });
        }
    }

    best_overlay
}

fn rasterize_overlays(
    width: usize,
    height: usize,
    points: &[PointMarker],
    rings: &[RingMarker],
    segments: &[RouteSegment],
) -> Vec<Option<DotSample>> {
    let mut raster = vec![None; width.saturating_mul(height)];
    if width == 0 || height == 0 {
        return raster;
    }

    for point in points {
        let radius = point.radius_sq.sqrt() + 0.5;
        for_each_pixel_in_bounds(width, height, point.x, point.y, radius, |x, y| {
            let dx = x as f64 + 0.5 - point.x;
            let dy = y as f64 + 0.5 - point.y;
            if dx * dx + dy * dy <= point.radius_sq {
                paint_overlay(
                    &mut raster,
                    width,
                    x,
                    y,
                    DotSample {
                        color: point.color,
                        priority: point.priority,
                    },
                );
            }
        });
    }

    for ring in rings {
        let outer_radius = ring.radius + 0.5;
        for_each_pixel_in_bounds(width, height, ring.x, ring.y, outer_radius, |x, y| {
            let distance = (x as f64 + 0.5 - ring.x).hypot(y as f64 + 0.5 - ring.y);
            if (distance - ring.radius).abs() <= 0.46 {
                paint_overlay(
                    &mut raster,
                    width,
                    x,
                    y,
                    DotSample {
                        color: ring.color,
                        priority: ring.priority,
                    },
                );
            }
        });
    }

    for segment in segments {
        let min_x = segment.x1.min(segment.x2);
        let max_x = segment.x1.max(segment.x2);
        let min_y = segment.y1.min(segment.y2);
        let max_y = segment.y1.max(segment.y2);
        let center_x = (min_x + max_x) * 0.5;
        let center_y = (min_y + max_y) * 0.5;
        let radius = ((max_x - min_x).max(max_y - min_y) * 0.5) + 1.0;
        for_each_pixel_in_bounds(width, height, center_x, center_y, radius, |x, y| {
            if (x as f64 + 0.5) < min_x - 0.6
                || (x as f64 + 0.5) > max_x + 0.6
                || (y as f64 + 0.5) < min_y - 0.6
                || (y as f64 + 0.5) > max_y + 0.6
            {
                return;
            }
            let distance = dist_to_segment_squared(
                x as f64 + 0.5,
                y as f64 + 0.5,
                segment.x1,
                segment.y1,
                segment.x2,
                segment.y2,
            );
            if distance <= 0.26 {
                paint_overlay(
                    &mut raster,
                    width,
                    x,
                    y,
                    DotSample {
                        color: segment.color,
                        priority: segment.priority,
                    },
                );
            }
        });
    }

    raster
}

fn for_each_pixel_in_bounds(
    width: usize,
    height: usize,
    center_x: f64,
    center_y: f64,
    radius: f64,
    mut draw: impl FnMut(usize, usize),
) {
    let min_x = (center_x - radius).floor().max(0.0) as usize;
    let max_x = (center_x + radius).ceil().min((width - 1) as f64) as usize;
    let min_y = (center_y - radius).floor().max(0.0) as usize;
    let max_y = (center_y + radius).ceil().min((height - 1) as f64) as usize;
    if min_x > max_x || min_y > max_y {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            draw(x, y);
        }
    }
}

fn route_sample_count(arc_angle: f64, radius_x: f64, radius_y: f64) -> usize {
    let projected_radius = (radius_x + radius_y) * 0.5;
    (arc_angle * projected_radius * 0.68)
        .ceil()
        .clamp(48.0, 128.0) as usize
}

fn paint_overlay(
    raster: &mut [Option<DotSample>],
    width: usize,
    x: usize,
    y: usize,
    sample: DotSample,
) {
    let current = &mut raster[y * width + x];
    if current.is_none_or(|value| sample.priority >= value.priority) {
        *current = Some(sample);
    }
}

fn distance_to_grid(value: f64, spacing: f64) -> f64 {
    (value - (value / spacing).round() * spacing).abs()
}

fn dist_to_segment_squared(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.0001 {
        return (px - x1).powi(2) + (py - y1).powi(2);
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    (px - proj_x).powi(2) + (py - proj_y).powi(2)
}

pub fn world_stipple(latitude: f64, longitude: f64) -> f64 {
    let lat_term = ((latitude + 90.0) * 12.9898).sin();
    let lon_term = ((longitude + 180.0) * 78.233).cos();
    ((lat_term * 43758.5453 + lon_term * 24634.63) % 1.0).abs()
}

const BITMAP_SIZE: usize = (MASK_WIDTH * MASK_HEIGHT) / 8;
static MASKS_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ne_50m_masks.bin"));

#[derive(Clone, Copy)]
struct MapSample {
    land: bool,
    coast: bool,
    boundary: bool,
}

pub fn geo_to_vec(latitude: f64, longitude: f64) -> DVec3 {
    let lat = latitude.to_radians();
    let lon = longitude.to_radians();
    DVec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos())
}

fn project_vec_camera(
    point: DVec3,
    rotation: f64,
    pitch: f64,
    center_x: f64,
    center_y: f64,
    radius_x: f64,
    radius_y: f64,
) -> Option<(f64, f64, f64)> {
    let x_prime = point.x * rotation.cos() - point.z * rotation.sin();
    let z_prime = point.x * rotation.sin() + point.z * rotation.cos();
    let y_prime = point.y;

    let x_double_prime = x_prime;
    let y_double_prime = y_prime * pitch.cos() - z_prime * pitch.sin();
    let z_double_prime = y_prime * pitch.sin() + z_prime * pitch.cos();

    if z_double_prime < 0.0 {
        return None;
    }

    Some((
        center_x + x_double_prime * radius_x,
        center_y - y_double_prime * radius_y,
        z_double_prime,
    ))
}

pub fn land_mask(latitude: f64, longitude: f64) -> bool {
    map_sample(latitude, longitude).land
}

fn map_sample(latitude: f64, longitude: f64) -> MapSample {
    let index = mask_index(latitude, longitude);
    let byte_offset = index / 8;
    let bit_mask = 1 << (index % 8);

    let land = (MASKS_BIN[byte_offset] & bit_mask) != 0;
    let coast = (MASKS_BIN[BITMAP_SIZE + byte_offset] & bit_mask) != 0;
    let boundary = (MASKS_BIN[BITMAP_SIZE * 2 + byte_offset] & bit_mask) != 0;

    MapSample {
        land,
        coast,
        boundary,
    }
}

fn mask_index(latitude: f64, longitude: f64) -> usize {
    let wrapped_longitude = (longitude + 180.0).rem_euclid(360.0);
    let x = ((wrapped_longitude / 360.0) * MASK_WIDTH as f64)
        .floor()
        .min((MASK_WIDTH - 1) as f64) as usize;
    let y = (((90.0 - latitude.clamp(-90.0, 90.0)) / 180.0) * MASK_HEIGHT as f64)
        .floor()
        .min((MASK_HEIGHT - 1) as f64) as usize;
    y * MASK_WIDTH + x
}

fn interpolate_arc_with_angle(start: DVec3, end: DVec3, amount: f64, angle: f64) -> DVec3 {
    if angle < 0.000_001 {
        return start.lerp(end, amount).normalize();
    }
    let sine = angle.sin();
    if sine.abs() < 0.000_001 {
        // Antipodal points have no unique shortest route. Pick a stable
        // orthogonal great-circle plane instead of allowing a near-zero
        // denominator to magnify floating-point error.
        let basis = if start.x.abs() <= start.y.abs() && start.x.abs() <= start.z.abs() {
            DVec3::X
        } else if start.y.abs() <= start.z.abs() {
            DVec3::Y
        } else {
            DVec3::Z
        };
        let orthogonal = start.cross(basis).normalize();
        let theta = angle * amount;
        return (start * theta.cos() + orthogonal * theta.sin()).normalize();
    }
    (start * ((1.0 - amount) * angle).sin() / sine + end * (amount * angle).sin() / sine)
        .normalize()
}

fn braille_glyph(bits: u8) -> char {
    char::from_u32(0x2800 + u32::from(bits)).unwrap_or(' ')
}

fn scale_color(color: [u8; 3], brightness: f64) -> [u8; 3] {
    [
        (f64::from(color[0]) * brightness) as u8,
        (f64::from(color[1]) * brightness) as u8,
        (f64::from(color[2]) * brightness) as u8,
    ]
}

fn smoother_fade(phase: f64) -> f64 {
    let remaining = (1.0 - phase).clamp(0.0, 1.0);
    remaining * remaining * (3.0 - 2.0 * remaining)
}

fn to_color(rgb: [u8; 3]) -> Color {
    Color::Rgb(rgb[0], rgb[1], rgb[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::Event;
    use ratatui::{Terminal, backend::TestBackend};

    const FIXTURE: &str = include_str!("../data/demo-topology.json");

    fn test_app() -> App {
        App::new(crate::model::Topology::from_json(FIXTURE).expect("fixture should parse"))
    }

    fn render_text(width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        let mut app = test_app();
        app.tick(std::time::Duration::from_secs(2));
        terminal
            .draw(|frame| render(frame, &app))
            .expect("representative deck should render");
        buffer_text(terminal.backend().buffer())
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let mut output = String::new();
        for y in buffer.area.top()..buffer.area.bottom() {
            for x in buffer.area.left()..buffer.area.right() {
                output.push_str(buffer[(x, y)].symbol());
            }
            output.push('\n');
        }
        output
    }

    #[test]
    fn geographic_origin_is_on_unit_sphere() {
        let point = geo_to_vec(0.0, 0.0);
        assert!((point.length() - 1.0).abs() < f64::EPSILON);
        assert!((point.z - 1.0).abs() < f64::EPSILON);
        assert!(point.x.abs() < f64::EPSILON);
        assert!(point.y.abs() < f64::EPSILON);
    }

    #[test]
    fn target_focus_heading_places_city_marker_on_visible_hemisphere() {
        let topology =
            crate::model::Topology::from_json(include_str!("../data/demo-topology.json"))
                .expect("fixture should parse");
        let target = &topology.targets[0];
        let rotation = target.location.longitude.to_radians();
        let pitch = target.location.latitude.to_radians();
        let projected = project_vec_camera(
            geo_to_vec(target.location.latitude, target.location.longitude),
            rotation,
            pitch,
            10.0,
            10.0,
            5.0,
            5.0,
        )
        .expect("focused target should be visible");
        assert!((projected.0 - 10.0).abs() < 0.001);
        assert!((projected.1 - 10.0).abs() < 0.001);
        assert!(projected.2 > 0.99);
    }

    #[test]
    fn east_and_west_project_to_correct_sides_of_screen() {
        let rot = 4.90_f64.to_radians();
        let pitch = 52.37_f64.to_radians();

        // Tokyo is East of Amsterdam (139.65E)
        let tokyo = geo_to_vec(35.68, 139.65);
        let proj_tokyo = project_vec_camera(tokyo, rot, pitch, 100.0, 100.0, 50.0, 50.0)
            .expect("Tokyo should be visible");
        assert!(
            proj_tokyo.0 > 100.0,
            "East (Tokyo) must be on the right side of the screen (>100.0), got {}",
            proj_tokyo.0
        );

        // Ashburn is West of Amsterdam (-77.49W)
        let ashburn = geo_to_vec(39.04, -77.49);
        let proj_ashburn = project_vec_camera(ashburn, rot, pitch, 100.0, 100.0, 50.0, 50.0)
            .expect("Ashburn should be visible");
        assert!(
            proj_ashburn.0 < 100.0,
            "West (Ashburn) must be on the left side of the screen (<100.0), got {}",
            proj_ashburn.0
        );
    }

    #[test]
    fn cached_masks_include_coasts_and_country_boundaries() {
        assert_eq!(MASKS_BIN.len(), BITMAP_SIZE * 3);
        assert!(MASKS_BIN[..BITMAP_SIZE].iter().any(|v| *v != 0));
        assert!(
            MASKS_BIN[BITMAP_SIZE..BITMAP_SIZE * 2]
                .iter()
                .any(|v| *v != 0)
        );
        assert!(MASKS_BIN[BITMAP_SIZE * 2..].iter().any(|v| *v != 0));
    }

    #[test]
    fn braille_dot_mapping_produces_unicode_braille() {
        let all_dots = BRAILLE_DOTS
            .iter()
            .fold(0_u8, |bits, (_, _, mask)| bits | mask);
        assert_eq!(all_dots, 0xff);
        assert_eq!(braille_glyph(all_dots), '\u{28ff}');
    }

    #[test]
    fn arc_interpolation_stays_on_unit_sphere() {
        let start = geo_to_vec(0.0, 0.0);
        let end = geo_to_vec(0.0, 90.0);
        let point = interpolate_arc_with_angle(start, end, 0.5, start.dot(end).acos());
        assert!((point.length() - 1.0).abs() < 0.000_001);
        assert!((point.x - point.z).abs() < 0.000_001);
    }

    #[test]
    fn land_mask_separates_europe_from_mid_atlantic() {
        assert!(land_mask(52.37, 4.90)); // Amsterdam
        assert!(land_mask(51.50, -0.12)); // London, UK
        assert!(land_mask(53.35, -6.26)); // Dublin, Ireland
        assert!(land_mask(35.68, 139.65)); // Tokyo, Japan
        assert!(land_mask(32.08, 34.78)); // Tel Aviv, Israel
        assert!(land_mask(39.04, -77.49)); // Ashburn, VA
        assert!(land_mask(-33.86, 151.20)); // Sydney, Australia
        assert!(land_mask(-23.55, -46.63)); // São Paulo, Brazil
        assert!(!land_mask(0.0, -30.0)); // Mid-Atlantic Ocean
        assert!(!land_mask(0.0, 160.0)); // Pacific Ocean
        assert!(!land_mask(-50.0, 0.0)); // South Atlantic Ocean
    }

    #[test]
    fn world_stipple_is_deterministic_and_bounded() {
        let s1 = world_stipple(52.37, 4.90);
        let s2 = world_stipple(52.37, 4.90);
        assert_eq!(s1, s2);
        assert!((0.0..=1.0).contains(&s1));
    }

    #[test]
    fn active_target_generates_compact_pulsing_beacon() {
        let app = test_app();
        let theme = get_theme(ThemeId::CyberOrbital);
        let geometry = GlobeGeometry {
            center_x: 50.0,
            center_y: 25.0,
            radius_x: 20.0,
            radius_y: 20.0,
            rotation: app.rotation(),
            rotation_cos: app.rotation().cos(),
            rotation_sin: app.rotation().sin(),
            pitch: app.pitch(),
            pitch_cos: app.pitch().cos(),
            pitch_sin: app.pitch().sin(),
        };

        let (points, rings, _segments) = build_overlays(&app, &geometry, &theme);
        assert_eq!(
            rings.len(),
            1,
            "Active target should have one compact pulse"
        );
        for r in &rings {
            assert!(
                r.radius >= 1.15 && r.radius <= 3.5,
                "Ring radius within compact bounds"
            );
        }
        let active_point = points
            .iter()
            .find(|point| point.priority == 4 && point.color == theme.active_target)
            .expect("active target should have a center point");
        assert!(
            active_point.radius_sq <= 0.64,
            "active target center should remain subpixel-sized"
        );
    }

    #[test]
    fn route_sample_budget_scales_with_geodesic_distance() {
        assert_eq!(route_sample_count(0.0, 50.0, 42.0), 48);
        assert!(route_sample_count(1.0, 50.0, 42.0) >= 48);
        assert!(route_sample_count(2.0, 50.0, 42.0) > route_sample_count(1.0, 50.0, 42.0));
        assert_eq!(route_sample_count(std::f64::consts::PI, 200.0, 200.0), 128);
    }

    #[test]
    fn route_reveal_keeps_packet_at_head_then_completes_path() {
        let mut app = test_app();
        app.tick(std::time::Duration::from_millis(400));
        assert!((0.0..1.0).contains(&app.route_progress()));
        let theme = get_theme(ThemeId::CyberOrbital);
        let geometry = GlobeGeometry {
            center_x: 80.0,
            center_y: 50.0,
            radius_x: 55.0,
            radius_y: 42.0,
            rotation: app.rotation(),
            rotation_cos: app.rotation().cos(),
            rotation_sin: app.rotation().sin(),
            pitch: app.pitch(),
            pitch_cos: app.pitch().cos(),
            pitch_sin: app.pitch().sin(),
        };
        let (partial_points, _rings, partial_segments) = build_overlays(&app, &geometry, &theme);
        assert!(!partial_segments.is_empty());
        assert!(partial_points.iter().any(|point| point.priority == 5));

        app.tick(std::time::Duration::from_secs(1));
        assert_eq!(app.route_progress(), 1.0);
        let (locked_points, _rings, locked_segments) = build_overlays(&app, &geometry, &theme);
        assert!(locked_segments.len() >= partial_segments.len());
        assert!(locked_points.iter().any(|point| point.priority == 5));
    }

    #[test]
    fn settled_route_starts_and_ends_at_projected_markers() {
        let app = {
            let mut app = test_app();
            app.tick(std::time::Duration::from_secs(2));
            app
        };
        let theme = get_theme(ThemeId::CyberOrbital);
        let geometry = GlobeGeometry {
            center_x: 80.0,
            center_y: 50.0,
            radius_x: 55.0,
            radius_y: 42.0,
            rotation: app.rotation(),
            rotation_cos: app.rotation().cos(),
            rotation_sin: app.rotation().sin(),
            pitch: app.pitch(),
            pitch_cos: app.pitch().cos(),
            pitch_sin: app.pitch().sin(),
        };
        let (_, _, segments) = build_overlays(&app, &geometry, &theme);
        let origin = geo_to_vec(
            app.topology().origin.location.latitude,
            app.topology().origin.location.longitude,
        );
        let destination = geo_to_vec(
            app.target().location.latitude,
            app.target().location.longitude,
        );
        let origin_screen = project_vec_camera(
            origin,
            geometry.rotation,
            geometry.pitch,
            geometry.center_x,
            geometry.center_y,
            geometry.radius_x,
            geometry.radius_y,
        )
        .expect("origin should be visible in the focused Amsterdam view");
        let destination_screen = project_vec_camera(
            destination,
            geometry.rotation,
            geometry.pitch,
            geometry.center_x,
            geometry.center_y,
            geometry.radius_x,
            geometry.radius_y,
        )
        .expect("target should be visible in the focused view");
        let first = segments
            .first()
            .expect("settled route should have segments");
        let last = segments.last().expect("settled route should have a tail");
        assert!((first.x1 - origin_screen.0).hypot(first.y1 - origin_screen.1) < 0.001);
        assert!((last.x2 - destination_screen.0).hypot(last.y2 - destination_screen.1) < 0.001);
    }

    #[test]
    fn command_deck_layout_preserves_globe_and_rail_at_target_sizes() {
        let compact = deck_layout(Rect::new(0, 0, 80, 24));
        assert_eq!(compact.header.height, 2);
        assert_eq!(compact.footer.height, 2);
        assert_eq!(compact.globe.width, 46);
        assert_eq!(compact.rail.width, 34);
        assert_eq!(compact.globe.right(), compact.rail.x);

        let standard = deck_layout(Rect::new(0, 0, 120, 40));
        assert_eq!(standard.header.height, 3);
        assert_eq!(standard.footer.height, 3);
        assert_eq!(standard.globe.width, 75);
        assert_eq!(standard.rail.width, 44);
        assert_eq!(standard.globe.right() + 1, standard.rail.x);

        let wide = deck_layout(Rect::new(0, 0, 160, 50));
        assert!(wide.globe.width > wide.rail.width * 2);
        assert_eq!(wide.rail.width, 52);
    }

    #[test]
    fn representative_terminal_sizes_render_all_command_modules() {
        for (width, height) in [(80, 24), (100, 30), (120, 40), (160, 50)] {
            let output = render_text(width, height);
            for expected in [
                "ACCESS ATLAS",
                "00 // ORBITAL VIEW",
                "01 // TARGET LOCK",
                "02 // ACCESS VECTOR",
                "03 // INSPECTION",
                "Europe micro VM",
            ] {
                assert!(
                    output.contains(expected),
                    "{width}x{height} render should contain {expected:?}"
                );
            }
            if std::env::var_os("ACCESS_ATLAS_DUMP").is_some() {
                println!("\n--- {width}x{height} ---\n{output}");
            }
        }

        let compact = render_text(80, 24);
        for expected in [
            "RTT 42MS // ORBITAL ICE",
            "UP 14d",
            "ROUTE   local-workstation",
            "location    Amsterdam",
        ] {
            assert!(compact.contains(expected), "compact deck lost {expected:?}");
        }
        assert!(!compact.contains("42msPALETTE"));
        let compact_location = compact
            .lines()
            .find(|line| line.contains("location"))
            .expect("compact inspection should include location");
        assert!(
            compact_location.contains("Amsterdam, NL (Eu…"),
            "compact detail value should signal truncation: {compact_location}"
        );

        let standard = render_text(120, 40);
        for expected in [
            "CHECK",
            "ACCESS VECTOR [RO]",
            "ABOUT",
            "NOTE",
            "packet_loss",
        ] {
            assert!(
                standard.contains(expected),
                "standard deck lost {expected:?}"
            );
        }
        for label in ["ABOUT", "NOTE"] {
            let line = standard
                .lines()
                .find(|line| line.contains(label))
                .unwrap_or_else(|| panic!("standard deck should include {label}"));
            assert!(
                line.contains('…'),
                "{label} should signal bounded truncation: {line}"
            );
        }
    }

    #[test]
    fn settled_terminal_redraws_after_shrink_and_grow_events() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        let mut app = test_app();
        app.tick(std::time::Duration::from_secs(2));
        terminal
            .draw(|frame| render(frame, &app))
            .expect("settled standard deck should render");
        app.mark_rendered();
        app.tick(std::time::Duration::from_secs(1));
        assert!(!app.needs_render());

        for (width, height) in [(80, 24), (120, 40)] {
            terminal.backend_mut().resize(width, height);
            app.handle_event(Event::Resize(width, height));
            assert!(app.needs_render(), "resize should invalidate settled frame");
            terminal
                .draw(|frame| render(frame, &app))
                .expect("resized deck should render");
            app.mark_rendered();

            let buffer = terminal.backend().buffer();
            assert_eq!(buffer.area, Rect::new(0, 0, width, height));
            let output = buffer_text(buffer);
            for expected in [
                "00 // ORBITAL VIEW",
                "02 // ACCESS VECTOR",
                "03 // INSPECTION",
            ] {
                assert!(
                    output.contains(expected),
                    "{width}x{height} redraw lost {expected:?}"
                );
            }
            assert!(!app.needs_render());
        }
    }

    #[test]
    fn overlay_raster_keeps_highest_priority_sample() {
        let points = [
            PointMarker {
                x: 4.5,
                y: 4.5,
                radius_sq: 2.0,
                color: [10, 20, 30],
                priority: 2,
            },
            PointMarker {
                x: 4.5,
                y: 4.5,
                radius_sq: 1.0,
                color: [220, 230, 240],
                priority: 5,
            },
        ];
        let raster = rasterize_overlays(10, 10, &points, &[], &[]);
        let center = raster[4 * 10 + 4].expect("center should be painted");
        assert_eq!(center.priority, 5);
        assert_eq!(center.color, [220, 230, 240]);
    }

    #[test]
    fn antipodal_arc_interpolation_remains_finite() {
        let start = DVec3::X;
        let end = -DVec3::X;
        let midpoint = interpolate_arc_with_angle(start, end, 0.5, std::f64::consts::PI);
        assert!((midpoint.length() - 1.0).abs() < 0.000_001);
        assert!(
            (interpolate_arc_with_angle(start, end, 1.0, std::f64::consts::PI) - end).length()
                < 0.000_001
        );
    }

    #[test]
    fn panel_wrapping_prefers_word_boundaries_and_marks_truncation() {
        assert_eq!(
            wrap_text_for_panel("local-workstation → gcp-iap → target", 20, 2),
            ["local-workstation →", "gcp-iap → target"]
        );
        assert_eq!(wrap_text_for_panel("abcdefghijk", 5, 2), ["abcde", "fghi…"]);
        assert_eq!(ellipsized_panel_text("abcdefghijk", 5), "abcd…");
        assert_eq!(ellipsized_panel_text("short", 5), "short");
    }

    #[test]
    fn theme_palettes_are_defined_for_all_variants() {
        for theme_id in [
            ThemeId::CyberOrbital,
            ThemeId::TacticalRadar,
            ThemeId::MinimalAtlas,
            ThemeId::AmberCrt,
            ThemeId::DeepSpace,
        ] {
            let palette = get_theme(theme_id);
            assert!(!palette.name.is_empty());
            assert!(compact_palette_name(palette.name).chars().count() <= 11);
        }
    }
}
