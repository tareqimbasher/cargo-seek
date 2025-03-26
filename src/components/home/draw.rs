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
use crate::components::home::enums::Focusable;
use crate::components::home::Home;
use crate::components::ux::{Button, State, BLUE, GRAY, ORANGE, PURPLE};
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

        let list_items: Vec<ListItem> = results
            .crates
            .iter()
            .map(|i| {
                let tag = if i.is_local {
                    "[local]"
                } else if i.is_installed {
                    "[installed]"
                } else {
                    ""
                };

                let name = i.name.to_string();
                let version = i.version.to_string();

                let mut white_space = area.width as i32 - name.len() as i32 - 27 - correction;
                if white_space < 0 {
                    white_space = 1;
                }

                let line = format!(
                    "{}{}{:>12}{:>15}",
                    name,
                    " ".repeat(white_space as usize),
                    tag,
                    version
                );

                ListItem::new(if i.is_local {
                    line.set_style(Style::default().fg(Color::LightCyan))
                } else if i.is_installed {
                    line.set_style(Style::default().fg(Color::LightMagenta))
                } else {
                    line.into()
                })
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
            .highlight_style(home.config.styles[&Mode::App]["accent"].bold())
            .highlight_symbol("> ");

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
    let pad = 25;

    let text = Text::from(vec![
        Line::from(vec!["SEARCH".bold()]),
        Line::from(vec![
            format!("{:<pad$}", "ENTER:").set_style(prop_style),
            "Search".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + s:").set_style(prop_style),
            "Sort".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + f:").set_style(prop_style),
            "Filter".bold(),
        ]),
        Line::default(),
        Line::from(vec!["NAVIGATION".bold()]),
        Line::from(vec![
            format!("{:<pad$}", "TAB:").set_style(prop_style),
            "Switch between boxes".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "ESC:").set_style(prop_style),
            "Go back to search; clear results".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + h:").set_style(prop_style),
            "Toggle this usage screen".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + z:").set_style(prop_style),
            "Suspend".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + c:").set_style(prop_style),
            "Quit".bold(),
        ]),
        Line::default(),
        Line::from(vec!["LIST".bold()]),
        Line::from(vec![
            format!("{:<pad$}", "Up/Down:").set_style(prop_style),
            "Scroll in crate list".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Home/End:").set_style(prop_style),
            "Go to first/last crate in list".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "a:").set_style(prop_style),
            "Add".bold(),
            " (WIP)".gray(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "r:").set_style(prop_style),
            "Remove".bold(),
            " (WIP)".gray(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "i:").set_style(prop_style),
            "Install".bold(),
            " (WIP)".gray(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "u:").set_style(prop_style),
            "Uninstall".bold(),
            " (WIP)".gray(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + o:").set_style(prop_style),
            "Open docs URL".bold(),
            " (WIP)".gray(),
        ]),
        Line::default(),
        Line::from(vec!["PAGING".bold()]),
        Line::from(vec![
            format!("{:<pad$}", "Left/Right:").set_style(prop_style),
            "Go backward/forward a page".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + Left/Right:").set_style(prop_style),
            "Go backward/forward 10 pages".bold(),
        ]),
        Line::from(vec![
            format!("{:<pad$}", "Ctrl + Home/End:").set_style(prop_style),
            "Go to first/last page".bold(),
        ]),
    ]);

    let block = Block::default()
        .title(" 📖 Usage ")
        .title_style(home.config.styles[&Mode::App]["title"])
        .padding(Padding::uniform(1))
        .borders(Borders::ALL);

    frame.render_widget(&block, area);

    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((0, 0)),
        block.inner(area),
    );

    Ok(())
}

fn render_crate_details(
    home: &Home,
    krate: &Crate,
    frame: &mut Frame,
    area: Rect,
) -> AppResult<()> {
    let details_focused = home.focused == Focusable::AddButton
        || home.focused == Focusable::InstallButton
        || home.focused == Focusable::ReadmeButton
        || home.focused == Focusable::DocsButton;

    let main_block = Block::default()
        .title(format!(" 🧐 {} ", krate.name))
        .title_style(home.config.styles[&Mode::App]["title"])
        .padding(Padding::uniform(1))
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

    let text = Text::from(vec![
        Line::from(vec![
            format!("{:<left_column_width$}", "Version:").set_style(prop_style),
            krate.version.to_string().into(),
        ]),
        Line::from(vec![
            format!("{:<left_column_width$}", "Latest Version:").set_style(prop_style),
            krate.max_version.clone().unwrap_or_default().into(),
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
            format!("{:<left_column_width$}", "crates.io Page:").set_style(prop_style),
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
        Line::from(vec![
            format!("{:<left_column_width$}", "Description:").set_style(prop_style),
            krate.description.clone().unwrap_or_default().bold(),
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
        Constraint::Length(10),
        Constraint::Length(1),
        Constraint::Length(10),
    ]);

    // Buttons row 1
    let [property_area, button1_area, _, button2_area] =
        buttons_row_layout.areas(buttons_row1_area);

    frame.render_widget(Text::from("Cargo:").set_style(prop_style), property_area);
    frame.render_widget(
        Button::new("Add")
            .theme(BLUE)
            .state(match home.focused == Focusable::AddButton {
                true => State::Selected,
                _ => State::Normal,
            }),
        button1_area,
    );
    frame.render_widget(
        Button::new("Install").theme(PURPLE).state(
            match home.focused == Focusable::InstallButton {
                true => State::Selected,
                _ => State::Normal,
            },
        ),
        button2_area,
    );

    // Buttons row 2
    let [property_area, button1_area, _, button2_area] =
        buttons_row_layout.areas(buttons_row2_area);

    frame.render_widget(Text::from("Links:").set_style(prop_style), property_area);

    let mut button_areas = vec![button1_area, button2_area];

    if krate.repository.is_some() {
        frame.render_widget(
            Button::new("README").theme(GRAY).state(
                match home.focused == Focusable::ReadmeButton {
                    true => State::Selected,
                    _ => State::Normal,
                },
            ),
            button_areas.remove(0),
        );
    }

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

fn center(
    area: Rect,
    horizontal: Constraint,
    vertical: Constraint,
) -> AppResult<Rect> {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    Ok(area)
}
