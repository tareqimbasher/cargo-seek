use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Styled, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, List, ListItem, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};

use crate::components::home::Home;
use crate::components::home::focusable::Focusable;
use crate::components::home::overlay::Overlay;
use crate::components::ux::{
    Button, GRAY, ORANGE, PURPLE, State, Theme, YELLOW, center, labeled_lines,
};
use crate::errors::AppResult;
use crate::search::Crate;
use crate::util::{format_number, format_short_age, format_size, get_relative_time};

const DETAILS_LABEL_WIDTH: usize = 25;

pub fn render(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let [left_col_area, right_col_area] = Layout::horizontal([
        Constraint::Percentage(home.left_column_width_percent),
        Constraint::Percentage(100 - home.left_column_width_percent),
    ])
    .areas(area);

    render_left(home, frame, left_col_area);
    render_right(home, frame, right_col_area);

    // Draw overlay last so the modal sits on top of everything.
    if let Some(overlay) = home.overlay.as_mut() {
        overlay.draw(frame, area);
    }

    Ok(())
}

fn render_left(home: &mut Home, frame: &mut Frame, area: Rect) {
    let [search_area, results_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);

    render_search(home, frame, search_area);
    render_results(home, frame, results_area);
}

fn render_search(home: &mut Home, frame: &mut Frame, area: Rect) {
    let spinner_width = if home.is_searching { 3 } else { 0 };

    let [search_area, spinner_area] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(spinner_width)]).areas(area);

    // The width of the input area, removing 2 for the width of the border on each side
    let scroll_width = search_area.width.saturating_sub(2);
    let input_scroll = home.input.visual_scroll(scroll_width as usize);
    let input = Paragraph::new(home.input.value())
        .scroll((0, input_scroll as u16))
        .block(
            Block::default()
                .title(" Search ")
                .borders(Borders::ALL)
                .border_style(match home.focused {
                    Focusable::Search => home.config.theme.accent_active,
                    _ => Style::default(),
                }),
        );
    frame.render_widget(input, search_area);

    if home.focused == Focusable::Search {
        // Make the cursor visible and ask ratatui to put it at the specified coordinates after rendering
        frame.set_cursor_position((
            // Put cursor past the end of the input text
            search_area.x
                + (home.input.visual_cursor().max(input_scroll) - input_scroll) as u16
                + 1,
            // Move one line down, from the border to the input line
            search_area.y + 1,
        ))
    }

    if home.is_searching {
        let throbber_border = Block::default().padding(Padding::uniform(1));
        frame.render_widget(&throbber_border, spinner_area);

        let throbber = throbber_widgets_tui::Throbber::default()
            .style(home.config.theme.throbber)
            .throbber_set(throbber_widgets_tui::BRAILLE_EIGHT)
            .use_type(throbber_widgets_tui::WhichUse::Spin);

        frame.render_stateful_widget(
            throbber,
            throbber_border.inner(spinner_area),
            &mut home.spinner_state,
        );
    }
}

fn render_results(home: &mut Home, frame: &mut Frame, area: Rect) {
    let dropdown_title = |label: String, active: bool| {
        let style = if active {
            home.config.theme.title
        } else {
            Style::default()
        };
        Line::from(format!(" ▼ {label} ").set_style(style)).right_aligned()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(match home.focused {
            Focusable::Results => home.config.theme.accent_active,
            _ => Style::default(),
        })
        .title(dropdown_title(
            home.scope.to_string(),
            matches!(home.overlay, Some(Overlay::Scope(_))),
        ))
        .title(dropdown_title(
            home.sort.to_string(),
            matches!(home.overlay, Some(Overlay::Sort(_))),
        ));

    if let Some(results) = home.search_results.as_mut() {
        let selected_index = results.selected_index();
        let correction = 2;

        let list_items: Vec<ListItem> = results
            .crates
            .iter()
            .map(|cr| {
                let tag = if cr.project_version.is_some() {
                    "+ "
                } else if cr.installed_version.is_some() {
                    "i "
                } else {
                    "  "
                };

                let name = &cr.name;
                let mut version = cr.version.to_string();

                // If metadata is not loaded, version might be the project or installed version
                // and not the latest version. In that case, we don't want to manipulate the
                // displayed version string
                if cr.is_metadata_loaded() {
                    if let Some(project_version) = &cr.project_version {
                        version = format!("{version} ({project_version})");
                    } else if let Some(installed_version) = &cr.installed_version {
                        version = format!("{version} ({installed_version})");
                    }
                }

                let mut white_space = area.width as i32
                    - name.len() as i32
                    - tag.len() as i32
                    - version.len() as i32
                    - correction;
                if white_space < 1 {
                    white_space = 1;
                }

                let details = format!("{}{}{}", name, " ".repeat(white_space as usize), version);

                let style = if cr.project_version.is_some() {
                    Style::default().fg(Color::LightCyan)
                } else if cr.installed_version.is_some() {
                    Style::default().fg(Color::LightMagenta)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![tag.bold(), details.into()]).set_style(style))
            })
            .collect();

        let items_in_prev_pages = results.items_before_current_page();
        let selected_item_num = selected_index.map_or(0, |ix| ix + 1);
        let selected_item_num_in_total = items_in_prev_pages + selected_item_num;
        let selected = results.selected();

        let list = List::new(list_items)
            .block(
                block
                    .title(format!(
                        " {}/{} ",
                        selected_item_num_in_total, results.total_count
                    ))
                    .title_bottom(
                        Line::from(format!(
                            " Page {}/{} ",
                            results.current_page(),
                            results.page_count(),
                        ))
                        .right_aligned(),
                    ),
            )
            // Selected row highlight style
            .highlight_style(if selected.is_some_and(|s| s.project_version.is_some()) {
                Style::default()
                    .bold()
                    .bg(Color::LightCyan)
                    .fg(Color::Black)
            } else if selected.is_some_and(|s| s.installed_version.is_some()) {
                Style::default()
                    .bold()
                    .bg(Color::LightMagenta)
                    .fg(Color::Black)
            } else {
                Style::default()
                    .bold()
                    .bg(home.config.theme.accent.fg.unwrap_or(Color::Yellow))
                    .fg(Color::Black)
            });

        frame.render_stateful_widget(list, area, &mut results.list_state);
    } else {
        frame.render_widget(block, area);
    }
}

fn render_right(home: &mut Home, frame: &mut Frame, area: Rect) {
    if home.show_help || home.search_results.is_none() {
        render_help(home, frame, area);
        return;
    }

    let has_selection = home.get_selected_search_result_item().is_some();

    if has_selection {
        render_crate_details(home, frame, area);
    } else {
        render_no_results(home, frame, area);
    }
}

fn render_help(home: &mut Home, frame: &mut Frame, area: Rect) {
    let header_style = Style::default().bold();
    let prop_style = home.config.theme.accent.bold();
    let desc_style = Style::default();

    const PAD: usize = 20;

    let text = Text::from(vec![
        Line::from(vec![
            format!("{:<PAD$}", "SYMBOLS:").set_style(header_style),
            "+ ".light_cyan().bold(),
            "added".set_style(desc_style),
            "   ".into(),
            "i ".light_magenta().bold(),
            "installed".set_style(desc_style),
        ]),
        Line::default(),
        Line::from(vec!["SEARCH".set_style(header_style)]),
        Line::from(vec![
            format!("{:<PAD$}", "Enter:").set_style(prop_style),
            "Run search".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + s:").set_style(prop_style),
            "Sort".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + a:").set_style(prop_style),
            "Search scope".set_style(desc_style),
        ]),
        Line::default(),
        Line::from(vec!["NAVIGATION".set_style(header_style)]),
        Line::from(vec![
            format!("{:<PAD$}", "TAB:").set_style(prop_style),
            "Switch between boxes".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "ESC:").set_style(prop_style),
            "Go back to search; again to clear results".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + Left/Right:").set_style(prop_style),
            "Change column width".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "PgUp/PgDn:").set_style(prop_style),
            "Scroll crate details".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + h:").set_style(prop_style),
            "Toggle this help screen".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + c:").set_style(prop_style),
            "Quit".set_style(desc_style),
        ]),
        Line::default(),
        Line::from(vec!["RESULTS".set_style(header_style)]),
        Line::from(vec![
            format!("{:<PAD$}", "a, r:").set_style(prop_style),
            "Add (pick features) / remove from project".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "i, u:").set_style(prop_style),
            "Install (pick features) / uninstall binary".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + d:").set_style(prop_style),
            "Open docs".set_style(desc_style),
        ]),
        Line::default(),
        Line::from(vec![
            format!("{:<PAD$}", "Up, Down:").set_style(prop_style),
            "Select crate in list".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Left, Right:").set_style(prop_style),
            "Go previous/next page".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Home, End:").set_style(prop_style),
            "Go to first/last crate in page".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + Home/End:").set_style(prop_style),
            "Go to first/last page".set_style(desc_style),
        ]),
    ]);

    let block = Block::default()
        .title(" 📖 Help ")
        .title_style(home.config.theme.title)
        .padding(Padding::uniform(1))
        .borders(Borders::ALL)
        .border_style(match home.focused {
            Focusable::Help => home.config.theme.accent_active,
            _ => Style::default(),
        });

    let inner = block.inner(area);
    frame.render_widget(&block, area);

    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    // `line_count` is the wrapped height at this width, so the scroll bound tracks
    // the terminal size and help-text edits rather than a hardcoded line count.
    home.max_help_scroll = paragraph
        .line_count(inner.width)
        .saturating_sub(inner.height as usize);
    home.vertical_help_scroll = home.vertical_help_scroll.min(home.max_help_scroll);

    frame.render_widget(
        paragraph.scroll((home.vertical_help_scroll as u16, 0)),
        inner,
    );
}

fn render_crate_details(home: &mut Home, frame: &mut Frame, area: Rect) {
    let Some(cr) = home
        .search_results
        .as_ref()
        .and_then(|results| results.selected())
        .cloned()
    else {
        return;
    };

    let details_focused = home.is_details_focused();
    let focused = home.focused;
    let prop_style = if details_focused {
        home.config.theme.accent_active
    } else {
        home.config.theme.accent
    }
    .bold();

    let show_docs = home.should_show_docs_button();
    let show_repo = home.should_show_repo_button();
    let show_cratesio = home.should_show_cratesio_button();
    let show_librs = home.should_show_librs_button();

    let main_block = Block::default()
        .title(format!(" 🧐 {} ", cr.name))
        .title_style(home.config.theme.title)
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL)
        .border_style(if details_focused {
            home.config.theme.accent_active
        } else {
            Style::default()
        });

    let inner = main_block.inner(area);

    // Details fill the pane and the button rows stay pinned to the bottom.
    let [details_area, buttons_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(4)]).areas(inner);

    // Reserve the last column for the scrollbar so wrapping is stable whether or not it's shown.
    let text_width = details_area.width.saturating_sub(1) as usize;
    let text = build_details_text(&cr, prop_style, DETAILS_LABEL_WIDTH, text_width);

    let line_count = text.lines.len();
    let viewport = details_area.height as usize;
    home.details_viewport_height = viewport;
    home.max_details_scroll = line_count.saturating_sub(viewport);
    home.details_scroll = home.details_scroll.min(home.max_details_scroll);

    frame.render_widget(&main_block, area);
    frame.render_widget(
        Paragraph::new(text).scroll((home.details_scroll as u16, 0)),
        details_area,
    );

    if line_count > viewport {
        let mut scrollbar_state = ScrollbarState::new(line_count)
            .viewport_content_length(viewport)
            .position(home.details_scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            details_area,
            &mut scrollbar_state,
        );
    }

    let [_, buttons_row1_area, _, buttons_row2_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(buttons_area);

    let buttons_row_layout = Layout::horizontal([
        Constraint::Length(DETAILS_LABEL_WIDTH as u16),
        Constraint::Length(12),
        Constraint::Length(1),
        Constraint::Length(12),
    ]);

    // Only the visible buttons are rendered, left-packed into the row's slots so a hidden button leaves no gap.
    let row1: Vec<(&str, Theme, Focusable)> = [
        show_docs.then_some(("Docs", ORANGE, Focusable::DocsButton)),
        show_repo.then_some(("Repository", GRAY, Focusable::RepositoryButton)),
    ]
    .into_iter()
    .flatten()
    .collect();

    let row2: Vec<(&str, Theme, Focusable)> = [
        show_cratesio.then_some(("crates.io", YELLOW, Focusable::CratesIoButton)),
        show_librs.then_some(("lib.rs", PURPLE, Focusable::LibRsButton)),
    ]
    .into_iter()
    .flatten()
    .collect();

    render_button_row(
        frame,
        &buttons_row_layout,
        buttons_row1_area,
        focused,
        &row1,
    );
    render_button_row(
        frame,
        &buttons_row_layout,
        buttons_row2_area,
        focused,
        &row2,
    );
}

/// Renders `buttons` left-packed into the row layout's two slots. `layout` is the shared four-slot
/// `[pad, slot, gap, slot]` row layout.
fn render_button_row(
    frame: &mut Frame,
    layout: &Layout,
    row_area: Rect,
    focused: Focusable,
    buttons: &[(&str, Theme, Focusable)],
) {
    let [_, first_slot, _, second_slot] = layout.areas(row_area);
    for (&(label, theme, target), slot) in buttons.iter().zip([first_slot, second_slot]) {
        let state = if focused == target {
            State::Selected
        } else {
            State::Normal
        };
        frame.render_widget(Button::new(label).theme(theme).state(state), slot);
    }
}

/// Builds the crate details body.
fn build_details_text(
    cr: &Crate,
    prop_style: Style,
    label_width: usize,
    text_width: usize,
) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let row = |lines: &mut Vec<Line<'static>>, label: &str, value: Vec<Span<'static>>| {
        lines.extend(labeled_lines(
            label,
            prop_style,
            value,
            label_width,
            text_width,
        ));
    };
    let plain = |value: String| vec![Span::raw(value)];

    // Overview
    row(
        &mut lines,
        "Description:",
        vec![Span::styled(
            cr.description.clone().unwrap_or_default(),
            Style::default().bold(),
        )],
    );
    row(
        &mut lines,
        "Categories:",
        plain(
            cr.categories
                .as_ref()
                .map(|c| c.join(", "))
                .unwrap_or_else(|| "Loading...".into()),
        ),
    );
    if let Some(keywords) = cr.keywords.as_ref().filter(|k| !k.is_empty()) {
        row(&mut lines, "Keywords:", plain(keywords.join(", ")));
    }
    lines.push(Line::default());

    // Versions & health
    row(&mut lines, "Stable Version:", plain(cr.version.clone()));
    if let Some(latest) = cr.max_version.as_ref().filter(|max| **max != cr.version) {
        row(&mut lines, "Latest Version:", plain(latest.clone()));
    }
    if let Some(project_version) = cr.project_version.as_ref() {
        lines.extend(labeled_lines(
            "Project Version:",
            Style::default().fg(Color::LightCyan).bold(),
            vec![Span::styled(
                project_version.clone(),
                Style::default().bold(),
            )],
            label_width,
            text_width,
        ));
    }
    if let Some(installed_version) = cr.installed_version.as_ref() {
        lines.extend(labeled_lines(
            "Installed Version:",
            Style::default().fg(Color::LightMagenta).bold(),
            vec![Span::styled(
                installed_version.clone(),
                Style::default().bold(),
            )],
            label_width,
            text_width,
        ));
    }
    if cr.is_metadata_loaded() {
        row(
            &mut lines,
            "Releases:",
            plain(format_number(Some(cr.version_count as u64))),
        );
    }
    if let Some(updated) = cr.updated_at {
        row(
            &mut lines,
            "Updated:",
            plain(format!(
                "{} ({})",
                updated.format("%d/%m/%Y %H:%M:%S (UTC)"),
                get_relative_time(updated, Utc::now())
            )),
        );
    }
    if let Some(created) = cr.created_at {
        row(
            &mut lines,
            "Created:",
            plain(created.format("%d/%m/%Y %H:%M:%S (UTC)").to_string()),
        );
    }
    if cr.latest_yanked {
        lines.extend(labeled_lines(
            "Warning:",
            Style::default().fg(Color::LightRed).bold(),
            vec![Span::styled(
                "latest version is yanked",
                Style::default().fg(Color::LightRed),
            )],
            label_width,
            text_width,
        ));
    }
    // A single release would just repeat the Stable Version line, so only list if there is more
    // than one release.
    if cr.recent_versions.len() > 1 {
        for (i, version) in cr.recent_versions.iter().enumerate() {
            let mut spans = vec![if i == 0 {
                Span::styled(format!("{:<label_width$}", "Recent Versions:"), prop_style)
            } else {
                Span::raw(" ".repeat(label_width))
            }];
            spans.push(Span::raw(format!(
                "{:<12}({})",
                version.num,
                format_short_age(version.created_at, Utc::now())
            )));
            if version.yanked {
                spans.push(Span::styled(
                    " yanked",
                    Style::default().fg(Color::LightRed),
                ));
            }
            lines.push(Line::from(spans));
        }
    }
    lines.push(Line::default());

    if let Some(license) = cr.license.as_ref() {
        row(&mut lines, "License:", plain(license.clone()));
    }
    if let Some(rust_version) = cr.rust_version.as_ref() {
        row(&mut lines, "MSRV:", plain(rust_version.clone()));
    }
    if let Some(size) = cr.crate_size {
        row(&mut lines, "Size:", plain(format_size(Some(size))));
    }
    row(&mut lines, "Features:", build_features_text(cr));
    lines.push(Line::default());

    // Downloads & authorship
    row(&mut lines, "Downloads:", plain(format_number(cr.downloads)));
    row(
        &mut lines,
        "Recent Downloads:",
        plain(format_number(cr.recent_downloads)),
    );
    if let Some(published_by) = cr.published_by.as_ref() {
        row(&mut lines, "Published by:", plain(published_by.clone()));
    }
    lines.push(Line::default());

    // Links
    row(
        &mut lines,
        "Home Page:",
        plain(cr.homepage.clone().unwrap_or_default()),
    );
    row(
        &mut lines,
        "Documentation:",
        plain(cr.documentation.clone().unwrap_or_default()),
    );
    row(
        &mut lines,
        "Repository:",
        plain(cr.repository.clone().unwrap_or_default()),
    );
    row(
        &mut lines,
        "crates.io:",
        plain(format!("https://crates.io/crates/{}", cr.id)),
    );

    Text::from(lines)
}

/// Builds the `Features:` text.
fn build_features_text(cr: &Crate) -> Vec<Span<'static>> {
    match cr.features.as_ref() {
        None => vec![Span::raw("Loading...")],
        Some(features) if features.is_empty() => {
            vec![Span::styled("(none)", Style::default().dim())]
        }
        Some(features) => {
            let mut spans = Vec::with_capacity(features.len() * 2);
            for (i, name) in features.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(", "));
                }
                spans.push(if cr.is_default_feature(name) {
                    Span::styled(name.clone(), Style::default().bold())
                } else {
                    Span::raw(name.clone())
                });
            }
            spans
        }
    }
}

fn render_no_results(home: &mut Home, frame: &mut Frame, area: Rect) {
    let main_block = Block::default()
        .title(" No results ")
        .title_style(home.config.theme.title)
        .padding(Padding::uniform(1))
        .borders(Borders::ALL);

    let text = Text::raw("0 crates found");
    let centered = center(
        main_block.inner(area),
        Constraint::Length(text.width() as u16),
        Constraint::Length(1),
    );

    frame.render_widget(main_block, area);
    frame.render_widget(text, centered);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::VersionSummary;
    use chrono::DateTime;
    use pretty_assertions::assert_eq;

    fn joined(spans: &[Span<'static>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn ver(num: &str, timestamp: i64, yanked: bool) -> VersionSummary {
        VersionSummary {
            num: num.into(),
            created_at: DateTime::from_timestamp(timestamp, 0).unwrap(),
            yanked,
        }
    }

    fn sample_crate() -> Crate {
        Crate {
            id: "chrono".into(),
            name: "chrono".into(),
            description: Some("Date and time library for Rust".into()),
            version: "0.4.45".into(),
            max_version: Some("0.4.45".into()),
            license: Some("MIT OR Apache-2.0".into()),
            rust_version: Some("1.62".into()),
            crate_size: Some(1_400_000),
            version_count: 142,
            published_by: Some("Xidorn Quan (quininer)".into()),
            features: Some((0..20).map(|i| format!("feature-{i:02}")).collect()),
            default_features: vec!["feature-00".into()],
            categories: Some(vec!["date-and-time".into()]),
            keywords: Some(vec![
                "datetime".into(),
                "timezone".into(),
                "calendar".into(),
            ]),
            recent_versions: vec![
                ver("0.4.45", 1_700_000_000, false),
                ver("0.4.44", 1_690_000_000, false),
                ver("0.4.43", 1_680_000_000, false),
            ],
            downloads: Some(651_595_646),
            recent_downloads: Some(130_877_048),
            homepage: Some("https://github.com/chronotope/chrono".into()),
            documentation: Some("https://docs.rs/chrono/".into()),
            repository: Some("https://github.com/chronotope/chrono".into()),
            metadata_loaded: true,
            ..Default::default()
        }
    }

    #[test]
    fn details_text_orders_groups_and_includes_new_fields() {
        let text = build_details_text(&sample_crate(), Style::default(), 25, 80);
        let body = text
            .lines
            .iter()
            .map(|line| joined(&line.spans))
            .collect::<Vec<_>>()
            .join("\n");

        for label in [
            "Description:",
            "Categories:",
            "Keywords:",
            "Stable Version:",
            "Releases:",
            "Recent Versions:",
            "License:",
            "MSRV:",
            "Size:",
            "Features:",
            "Downloads:",
            "Published by:",
            "Home Page:",
        ] {
            assert!(body.contains(label), "details missing {label}");
        }

        let pos = |needle: &str| body.find(needle).expect(needle);
        assert!(pos("Description:") < pos("Categories:"));
        assert!(pos("Categories:") < pos("Keywords:"));
        assert!(pos("Keywords:") < pos("Stable Version:"));
        assert!(pos("Stable Version:") < pos("Recent Versions:"));
        assert!(pos("Recent Versions:") < pos("License:"));
        assert!(pos("License:") < pos("Features:"));
        assert!(pos("Features:") < pos("Downloads:"));
        assert!(pos("Downloads:") < pos("Home Page:"));

        // Latest Version is hidden when it equals the stable version.
        assert!(!body.contains("Latest Version:"));
        assert!(body.contains("1.3 MiB"));
    }

    #[test]
    fn details_text_lists_recent_versions_with_a_single_label_and_yank_marker() {
        let mut cr = sample_crate();
        cr.latest_yanked = false;
        cr.recent_versions = vec![
            ver("1.0.0", 1_700_000_000, false),
            ver("0.9.0", 1_690_000_000, true),
            ver("0.8.0", 1_680_000_000, false),
        ];

        let rendered = build_details_text(&cr, Style::default(), 25, 80)
            .lines
            .iter()
            .map(|line| joined(&line.spans))
            .collect::<Vec<_>>();

        let label_ix = rendered
            .iter()
            .position(|line| line.starts_with("Recent Versions:"))
            .expect("Recent Versions row");
        assert_eq!(
            rendered
                .iter()
                .filter(|line| line.contains("Recent Versions:"))
                .count(),
            1
        );
        assert!(rendered[label_ix].contains("1.0.0"));
        assert!(rendered[label_ix + 1].starts_with(&" ".repeat(25)));
        assert!(rendered[label_ix + 1].contains("0.9.0"));

        // latest_yanked is off, so the only "yanked" marker must come from the 0.9.0 release.
        let yanked_line = &rendered[label_ix + 1];
        assert!(
            yanked_line.contains("yanked"),
            "missing yank marker: {yanked_line:?}"
        );
    }

    #[test]
    fn details_text_wraps_features_with_hanging_indent() {
        let text = build_details_text(&sample_crate(), Style::default(), 25, 45);
        let rendered = text
            .lines
            .iter()
            .map(|line| joined(&line.spans))
            .collect::<Vec<_>>();

        for line in &rendered {
            assert!(line.chars().count() <= 45, "line too wide: {line:?}");
        }

        let features_ix = rendered
            .iter()
            .position(|line| line.starts_with("Features:"))
            .expect("Features row");
        let continuation = &rendered[features_ix + 1];
        assert!(
            continuation.starts_with(&" ".repeat(25)),
            "feature wrap not hang-indented: {continuation:?}"
        );
        assert!(continuation.trim_start().starts_with("feature-"));
    }
}
