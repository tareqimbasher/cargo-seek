use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Style, Styled, Stylize},
    text::{Line, Text},
    widgets::{
        Block, Borders, List, ListItem, Padding, Paragraph, Wrap,
        block::{Position, Title},
    },
};

use crate::app::Mode;
use crate::components::Component;
use crate::components::home::Home;
use crate::components::home::focusable::Focusable;
use crate::components::ux::{Button, GRAY, ORANGE, PURPLE, State, YELLOW};
use crate::errors::AppResult;
use crate::search::Crate;
use crate::util::{format_number, get_relative_time};

pub fn render(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let [left_col_area, right_col_area] = Layout::horizontal([
        Constraint::Percentage(home.left_column_width_percent),
        Constraint::Percentage(100 - home.left_column_width_percent),
    ])
    .areas(area);

    render_left(home, frame, left_col_area)?;
    render_right(home, frame, right_col_area)?;
    Ok(())
}

fn render_left(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let [search_area, results_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);

    render_search(home, frame, search_area)?;
    render_results(home, frame, results_area)?;
    home.scope_dropdown.draw(&Mode::Home, frame, area)?;
    home.sort_dropdown.draw(&Mode::Home, frame, area)?;

    Ok(())
}

fn render_search(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
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
                    Focusable::Search => home.config.styles[&Mode::App]["accent_active"],
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
            .style(home.config.styles[&Mode::App]["throbber"])
            .throbber_set(throbber_widgets_tui::BRAILLE_EIGHT)
            .use_type(throbber_widgets_tui::WhichUse::Spin);

        frame.render_stateful_widget(
            throbber,
            throbber_border.inner(spinner_area),
            &mut home.spinner_state,
        );
    }

    Ok(())
}

fn render_results(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(match home.focused {
            Focusable::Results => home.config.styles[&Mode::App]["accent_active"],
            _ => Style::default(),
        })
        .title(
            Title::from(
                format!(" ▼ {} ", home.scope_dropdown.get_selected()).set_style(
                    if home.focused == Focusable::Scope {
                        home.config.styles[&Mode::App]["title"]
                    } else {
                        Style::default()
                    },
                ),
            )
            .alignment(Alignment::Right),
        )
        .title(
            Title::from(
                format!(" ▼ {} ", home.sort_dropdown.get_selected()).set_style(
                    if home.focused == Focusable::Sort {
                        home.config.styles[&Mode::App]["title"]
                    } else {
                        Style::default()
                    },
                ),
            )
            .alignment(Alignment::Right),
        );

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

                let name = cr.name.to_string();
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

        let current_page = results.current_page();
        let items_in_prev_pages = if current_page < 1 {
            0
        } else {
            (current_page - 1) * 100
        };

        let selected_item_num = match selected_index {
            None => 0,
            Some(ix) => {
                if ix == usize::MAX {
                    results.current_page_count()
                } else if ix == usize::MIN {
                    1
                } else if ix > results.current_page_count() - 1 {
                    // ListState select_next() increments selected even after last item is selected
                    ix
                } else {
                    ix + 1
                }
            }
        };

        let selected_item_num_in_total = items_in_prev_pages + selected_item_num;
        let selected = results.selected();

        let list = List::new(list_items)
            .block(
                block
                    .title(Title::from(format!(
                        " {}/{} ",
                        selected_item_num_in_total, results.total_count
                    )))
                    .title(
                        Title::from(format!(
                            " Page {}/{} ",
                            results.current_page(),
                            results.page_count(),
                        ))
                        .position(Position::Bottom)
                        .alignment(Alignment::Right),
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
                    .bg(home.config.styles[&Mode::App]["accent"]
                        .fg
                        .unwrap_or(Color::Yellow))
                    .fg(Color::Black)
            });

        frame.render_stateful_widget(list, area, &mut results.list_state);
    } else {
        frame.render_widget(block, area);
    }

    Ok(())
}

fn render_right(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    if home.show_usage || home.search_results.is_none() {
        render_usage(home, frame, area)?;
        return Ok(());
    }

    let selected_crate = {
        let search_results = home.search_results.as_ref().unwrap();
        search_results.selected()
    };

    if let Some(cr) = selected_crate {
        render_crate_details(home, cr, frame, area)?;
    } else {
        render_no_results(home, frame, area)?;
    }

    Ok(())
}

fn render_usage(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let header_style = Style::default().bold();
    let prop_style = home.config.styles[&Mode::App]["accent"].bold();
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
            format!("{:<PAD$}", "Ctrl + h:").set_style(prop_style),
            "Toggle this usage screen".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + c:").set_style(prop_style),
            "Quit".set_style(desc_style),
        ]),
        Line::default(),
        Line::from(vec!["RESULTS".set_style(header_style)]),
        Line::from(vec![
            format!("{:<PAD$}", "a, r:").set_style(prop_style),
            "Add/remove to current project".set_style(desc_style),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "i, u:").set_style(prop_style),
            "Install/uninstall binary".set_style(desc_style),
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
        .title(" 📖 Usage ")
        .title_style(home.config.styles[&Mode::App]["title"])
        .padding(Padding::uniform(1))
        .borders(Borders::ALL)
        .border_style(match home.focused {
            Focusable::Usage => home.config.styles[&Mode::App]["accent_active"],
            _ => Style::default(),
        });

    frame.render_widget(&block, area);

    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((home.vertical_usage_scroll as u16, 0)),
        block.inner(area),
    );

    // let paragraph = Paragraph::new(text.clone())
    //     .gray()
    //     .block(block)
    //     .scroll((home.vertical_usage_scroll as u16, 0));
    // frame.render_widget(paragraph, area);
    // frame.render_stateful_widget(
    //     Scrollbar::new(ScrollbarOrientation::VerticalRight)
    //         .begin_symbol(Some("↑"))
    //         .end_symbol(Some("↓")),
    //     area,
    //     &mut home.vertical_usage_scroll_state,
    // );

    Ok(())
}

fn render_crate_details(home: &Home, cr: &Crate, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let details_focused = home.focused == Focusable::DocsButton
        || home.focused == Focusable::RepositoryButton
        || home.focused == Focusable::CratesIoButton
        || home.focused == Focusable::LibRsButton;

    let main_block = Block::default()
        .title(format!(" 🧐 {} ", cr.name))
        .title_style(home.config.styles[&Mode::App]["title"])
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL)
        .border_style(if details_focused {
            home.config.styles[&Mode::App]["accent_active"]
        } else {
            Style::default()
        });

    let left_column_width = 25;

    let prop_style = home.config.styles[&Mode::App][if details_focused {
        "accent_active"
    } else {
        "accent"
    }]
    .bold();

    let mut text = Text::default();

    text.lines.extend(vec![
        Line::from(vec![
            format!("{:<left_column_width$}", "Stable Version:").set_style(prop_style),
            cr.version.to_string().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Latest Version:").set_style(prop_style),
            cr.max_version.clone().unwrap_or_default().into(),
        ]),
    ]);

    if let Some(project_version) = &cr.project_version {
        text.lines.push(Line::from(vec![
            format!("{:<left_column_width$}", "Project Version:")
                .light_cyan()
                .bold(),
            project_version.to_string().bold(),
        ]));
    }

    if let Some(installed_version) = &cr.installed_version {
        text.lines.push(Line::from(vec![
            format!("{:<left_column_width$}", "Installed Version:")
                .light_magenta()
                .bold(),
            installed_version.to_string().bold(),
        ]));
    }

    text.lines.extend(vec![
        Line::from(vec![
            format!("{:<left_column_width$}", "Description:").set_style(prop_style),
            cr.description.clone().unwrap_or_default().bold(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Home Page:").set_style(prop_style),
            cr.homepage.clone().unwrap_or_default().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Documentation:").set_style(prop_style),
            cr.documentation.clone().unwrap_or_default().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Repository:").set_style(prop_style),
            cr.repository.clone().unwrap_or_default().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "crates.io:").set_style(prop_style),
            format!("https://crates.io/crates/{}", cr.id).into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Downloads:").set_style(prop_style),
            format_number(cr.downloads).into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Recent Downloads:").set_style(prop_style),
            format_number(cr.recent_downloads).into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Features:").set_style(prop_style),
            cr.features
                .as_ref()
                .map(|v| v.join(", "))
                .unwrap_or("Loading...".into())
                .into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Categories:").set_style(prop_style),
            cr.categories
                .as_ref()
                .map(|v| v.join(", "))
                .unwrap_or("Loading...".into())
                .into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Created:").set_style(prop_style),
            match cr.created_at.as_ref() {
                None => "".into(),
                Some(v) => v.format("%d/%m/%Y %H:%M:%S (UTC)").to_string().into(),
            },
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Updated:").set_style(prop_style),
            match cr.updated_at.as_ref() {
                None => "".into(),
                Some(v) => {
                    let updated_relative = match cr.updated_at {
                        None => "".into(),
                        Some(v) => get_relative_time(v, Utc::now()),
                    };

                    format!(
                        "{} ({})",
                        v.format("%d/%m/%Y %H:%M:%S (UTC)"),
                        updated_relative
                    )
                    .into()
                }
            },
        ]),
    ]);

    let details_paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    frame.render_widget(&main_block, area);

    let [details_area, _, buttons_row1_area, _, buttons_row2_area] = Layout::vertical([
        Constraint::Max(20),   // details
        Constraint::Length(1), // empty line
        Constraint::Length(1), // buttons row 1
        Constraint::Length(1), // empty line
        Constraint::Length(1), // buttons row 2
    ])
    .areas(main_block.inner(area));

    frame.render_widget(details_paragraph, details_area);

    let buttons_row_layout = Layout::horizontal([
        Constraint::Length(left_column_width as u16),
        Constraint::Length(12),
        Constraint::Length(1),
        Constraint::Length(12),
    ]);

    // Button row 1
    let [_, button1_area, _, button2_area] = buttons_row_layout.areas(buttons_row1_area);

    let mut button_areas = vec![button1_area, button2_area];

    if cr.documentation.is_some() {
        frame.render_widget(
            Button::new("Docs")
                .theme(ORANGE)
                .state(match home.focused == Focusable::DocsButton {
                    true => State::Selected,
                    _ => State::Normal,
                }),
            button_areas.remove(0),
        );
    }

    if cr.repository.is_some() {
        frame.render_widget(
            Button::new("Repository").theme(GRAY).state(
                match home.focused == Focusable::RepositoryButton {
                    true => State::Selected,
                    _ => State::Normal,
                },
            ),
            button_areas.remove(0),
        );
    }

    // Button row 2
    let [_, button1_area, _, button2_area] = buttons_row_layout.areas(buttons_row2_area);

    frame.render_widget(
        Button::new("crates.io").theme(YELLOW).state(
            match home.focused == Focusable::CratesIoButton {
                true => State::Selected,
                _ => State::Normal,
            },
        ),
        button1_area,
    );
    frame.render_widget(
        Button::new("lib.rs")
            .theme(PURPLE)
            .state(match home.focused == Focusable::LibRsButton {
                true => State::Selected,
                _ => State::Normal,
            }),
        button2_area,
    );

    Ok(())
}

fn render_no_results(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let main_block = Block::default()
        .title(" No results ")
        .title_style(home.config.styles[&Mode::App]["title"])
        .padding(Padding::uniform(1))
        .borders(Borders::ALL);

    let text = Text::raw("0 crates found");
    let centered = center(
        main_block.inner(area),
        Constraint::Length(text.width() as u16),
        Constraint::Length(1),
    )?;

    frame.render_widget(main_block, area);
    frame.render_widget(text, centered);

    Ok(())
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> AppResult<Rect> {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    Ok(area)
}
