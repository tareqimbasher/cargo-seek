use chrono::Utc;
use ratatui::prelude::{Color, Line, Text};
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap};
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Style, Styled, Stylize},
    Frame,
};

use crate::app::Mode;
use crate::components::home::focusable::Focusable;
use crate::components::home::Home;
use crate::components::ux::{Button, State, GRAY, ORANGE, PURPLE, YELLOW};
use crate::components::Component;
use crate::errors::AppResult;
use crate::search::Crate;
use crate::util::Util;

pub fn render(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area);

    render_left(home, frame, left)?;
    render_right(home, frame, right)?;
    Ok(())
}

fn render_left(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let [search, results] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);

    render_search(home, frame, search)?;
    render_results(home, frame, results)?;
    home.scope_dropdown.draw(&Mode::Home, frame, area)?;
    home.sort_dropdown.draw(&Mode::Home, frame, area)?;

    Ok(())
}

fn render_search(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let spinner_len = if home.is_searching { 3 } else { 0 };

    let [search, spinner] =
        Layout::horizontal([Constraint::Min(1), Constraint::Length(spinner_len)]).areas(area);

    // The width of the input area, removing 2 for the width of the border on each side
    let scroll_width = if search.width < 2 {
        0
    } else {
        search.width - 2
    };
    let scroll = home.input.visual_scroll(scroll_width as usize);
    let input = Paragraph::new(home.input.value())
        .scroll((0, scroll as u16))
        .block(
            Block::default()
                .title(" Search ")
                .borders(Borders::ALL)
                .border_style(match home.focused {
                    Focusable::Search => home.config.styles[&Mode::App]["accent_active"],
                    _ => Style::default(),
                }),
        );
    frame.render_widget(input, search);

    if home.focused == Focusable::Search {
        // Make the cursor visible and ask ratatui to put it at the specified coordinates after rendering
        frame.set_cursor_position((
            // Put cursor past the end of the input text
            search.x + (home.input.visual_cursor().max(scroll) - scroll) as u16 + 1,
            // Move one line down, from the border to the input line
            search.y + 1,
        ))
    }

    if home.is_searching {
        let throbber_border = Block::default().padding(Padding::uniform(1));
        frame.render_widget(&throbber_border, spinner);

        let throbber = throbber_widgets_tui::Throbber::default()
            .style(home.config.styles[&Mode::App]["throbber"])
            .throbber_set(throbber_widgets_tui::BRAILLE_EIGHT)
            .use_type(throbber_widgets_tui::WhichUse::Spin);

        frame.render_stateful_widget(
            throbber,
            throbber_border.inner(spinner),
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
        let correction = match results.get_selected_index() {
            Some(_) => 4,
            None => 2,
        };

        const VERSION_PADDING: usize = 15;

        let list_items: Vec<ListItem> = results
            .crates
            .iter()
            .map(|i| {
                let tag = if i.local_version.is_some() {
                    "+ "
                } else if i.installed_version.is_some() {
                    "i "
                } else {
                    "  "
                };

                let name = i.name.to_string();
                let version = i.version.to_string();

                let mut white_space = area.width as i32
                    - name.len() as i32
                    - tag.len() as i32
                    - VERSION_PADDING as i32
                    - correction;
                if white_space < 1 {
                    white_space = 1;
                }

                let line = format!(
                    "{}{}{:>VERSION_PADDING$}",
                    name,
                    " ".repeat(white_space as usize),
                    version
                );

                let style = if i.local_version.is_some() {
                    Style::default().fg(Color::LightCyan)
                } else if i.installed_version.is_some() {
                    Style::default().fg(Color::LightMagenta)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![tag.bold(), line.into()]).set_style(style))
            })
            .collect();

        let items_in_prev_pages = match results.current_page() {
            1 => 0,
            p => {
                if p < 1 {
                    0
                } else {
                    (p - 1) * 100
                }
            }
        };

        let selected_item_num = match results.get_selected_index() {
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
            //.highlight_style(home.config.styles[&Mode::App]["accent"].bold())
            .highlight_style(Style::default().bold())
            .highlight_symbol(">");

        frame.render_stateful_widget(list, area, results.list_state());
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
        search_results.get_selected()
    };

    if let Some(krate) = selected_crate {
        render_crate_details(home, krate, frame, area)?;
    } else {
        render_no_results(home, frame, area)?;
    }

    Ok(())
}

fn render_usage(home: &mut Home, frame: &mut Frame, area: Rect) -> AppResult<()> {
    let prop_style = home.config.styles[&Mode::App]["accent"].bold();
    const PAD: usize = 20;

    let text = Text::from(vec![
        Line::from(vec![
            format!("{:<PAD$}", "SYMBOLS:").bold(),
            "+ ".light_cyan().bold(),
            "added".bold(),
            "   ".into(),
            "i ".light_magenta().bold(),
            "installed".bold(),
        ]),
        Line::default(),
        Line::from(vec!["SEARCH".bold()]),
        Line::from(vec![
            format!("{:<PAD$}", "Enter:").set_style(prop_style),
            "Search".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + s:").set_style(prop_style),
            "Sort".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + f:").set_style(prop_style),
            "Filter".bold(),
        ]),
        Line::default(),
        Line::from(vec!["NAVIGATION".bold()]),
        Line::from(vec![
            format!("{:<PAD$}", "TAB:").set_style(prop_style),
            "Switch between boxes".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "ESC:").set_style(prop_style),
            "Go back to search; again to clear results".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + h:").set_style(prop_style),
            "Toggle this usage screen".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + z:").set_style(prop_style),
            "Suspend".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + c:").set_style(prop_style),
            "Quit".bold(),
        ]),
        Line::default(),
        Line::from(vec!["RESULTS".bold()]),
        Line::from(vec![
            format!("{:<PAD$}", "Up, Down:").set_style(prop_style),
            "Scroll in crate list".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Home, End:").set_style(prop_style),
            "Go to first/last crate in list".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "a, r:").set_style(prop_style),
            "Add/remove to current project".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "i, u:").set_style(prop_style),
            "Install/uninstall binary".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + d:").set_style(prop_style),
            "Open docs".bold(),
        ]),
        Line::default(),
        Line::from(vec!["PAGING".bold()]),
        Line::from(vec![
            format!("{:<PAD$}", "Left, Right:").set_style(prop_style),
            "Go backward/forward a page".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + Left/Right:").set_style(prop_style),
            "Go backward/forward 10 pages".bold(),
        ]),
        Line::from(vec![
            format!("{:<PAD$}", "Ctrl + Home/End:").set_style(prop_style),
            "Go to first/last page".bold(),
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

fn render_crate_details(
    home: &Home,
    krate: &Crate,
    frame: &mut Frame,
    area: Rect,
) -> AppResult<()> {
    let details_focused = home.focused == Focusable::DocsButton
        || home.focused == Focusable::ReadmeButton
        || home.focused == Focusable::CratesIoButton
        || home.focused == Focusable::LibRsButton;

    let main_block = Block::default()
        .title(format!(" 🧐 {} ", krate.name))
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
            format!("{:<left_column_width$}", "Version:").set_style(prop_style),
            krate.version.to_string().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Latest Version:").set_style(prop_style),
            krate.max_version.clone().unwrap_or_default().into(),
        ]),
    ]);

    if let Some(local_version) = &krate.local_version {
        text.lines.push(Line::from(vec![
            format!("{:<left_column_width$}", "Project Version:")
                .light_cyan()
                .bold(),
            local_version.to_string().bold(),
        ]));
    }

    if let Some(installed_version) = &krate.installed_version {
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
            krate.description.clone().unwrap_or_default().bold(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Home Page:").set_style(prop_style),
            krate.homepage.clone().unwrap_or_default().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Documentation:").set_style(prop_style),
            krate.documentation.clone().unwrap_or_default().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Repository:").set_style(prop_style),
            krate.repository.clone().unwrap_or_default().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "crates.io:").set_style(prop_style),
            format!("https://crates.io/crates/{}", krate.id).into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Downloads:").set_style(prop_style),
            Util::format_number(krate.downloads).into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Recent Downloads:").set_style(prop_style),
            Util::format_number(krate.recent_downloads).into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Created:").set_style(prop_style),
            match krate.created_at.as_ref() {
                None => "".into(),
                Some(v) => v.format("%d/%m/%Y %H:%M:%S (UTC)").to_string().into(),
            },
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Updated:").set_style(prop_style),
            match krate.created_at.as_ref() {
                None => "".into(),
                Some(v) => {
                    let updated_relative = match krate.updated_at {
                        None => "".into(),
                        Some(v) => Util::get_relative_time(v, Utc::now()),
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

    let details_paragraph_lines = text.lines.len();
    let details_paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

    frame.render_widget(&main_block, area);

    let [details_area, _, buttons_row1_area, _, buttons_row2_area] = Layout::vertical([
        Constraint::Length((details_paragraph_lines + 1) as u16),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(main_block.inner(area));

    frame.render_widget(details_paragraph, details_area);

    let buttons_row_layout = Layout::horizontal([
        Constraint::Length(left_column_width as u16),
        Constraint::Length(12),
        Constraint::Length(1),
        Constraint::Length(12),
    ]);

    // Buttons row 1
    let [_, button1_area, _, button2_area] = buttons_row_layout.areas(buttons_row1_area);

    let mut button_areas = vec![button1_area, button2_area];

    if krate.documentation.is_some() {
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

    if krate.repository.is_some() {
        frame.render_widget(
            Button::new("Repository").theme(GRAY).state(
                match home.focused == Focusable::ReadmeButton {
                    true => State::Selected,
                    _ => State::Normal,
                },
            ),
            button_areas.remove(0),
        );
    }

    // Buttons row 2
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
