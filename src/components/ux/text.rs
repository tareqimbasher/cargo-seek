use ratatui::prelude::{Line, Span, Style};

/// Lays out a labelled value: the label fills the left `label_width` columns; the value is
/// word-wrapped into the remaining width, and every continuation line is indented to sit
/// under the value column.
pub fn labeled_lines(
    label: &str,
    label_style: Style,
    value: Vec<Span<'static>>,
    label_width: usize,
    total_width: usize,
) -> Vec<Line<'static>> {
    let content_width = total_width.saturating_sub(label_width).max(1);

    wrap_value(value, content_width)
        .into_iter()
        .enumerate()
        .map(|(i, mut spans)| {
            let mut line = Vec::with_capacity(spans.len() + 1);
            if i == 0 {
                line.push(Span::styled(format!("{label:<label_width$}"), label_style));
            } else {
                line.push(Span::raw(" ".repeat(label_width)));
            }
            line.append(&mut spans);
            Line::from(line)
        })
        .collect()
}

/// Word-wraps a styled value into lines no wider than `width`, preserving each span's styling and
/// breaking at spaces. A single word longer than `width` is hard-broken so nothing can overflow.
fn wrap_value(spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);

    // Split into words carrying their styling. A word holds no spaces but may span several styled
    // pieces (ex: a bold feature name immediately followed by its plain separator).
    let mut words: Vec<Vec<Span<'static>>> = Vec::new();
    let mut word: Vec<Span<'static>> = Vec::new();
    for span in spans {
        let style = span.style;
        let mut piece = String::new();
        for ch in span.content.chars() {
            if ch == ' ' {
                if !piece.is_empty() {
                    word.push(Span::styled(std::mem::take(&mut piece), style));
                }
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            } else {
                piece.push(ch);
            }
        }
        if !piece.is_empty() {
            word.push(Span::styled(piece, style));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }

    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0;

    for word in words {
        let word_width = span_width(&word);

        if word_width > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            let mut chunks = hard_break(word, width);
            if let Some(last) = chunks.pop() {
                lines.extend(chunks);
                current_width = span_width(&last);
                current = last;
            }
            continue;
        }

        let needed = if current.is_empty() {
            word_width
        } else {
            word_width + 1
        };
        if !current.is_empty() && current_width + needed > width {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        if !current.is_empty() {
            current.push(Span::raw(" "));
            current_width += 1;
        }
        current.extend(word);
        current_width += word_width;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

/// Splits a single long word into chunks no wider than `width`, preserving styling.
fn hard_break(word: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut chunks: Vec<Vec<Span<'static>>> = Vec::new();
    let mut chunk: Vec<Span<'static>> = Vec::new();
    let mut chunk_width = 0;

    for span in word {
        let style = span.style;
        let mut piece = String::new();
        for ch in span.content.chars() {
            if chunk_width == width {
                if !piece.is_empty() {
                    chunk.push(Span::styled(std::mem::take(&mut piece), style));
                }
                chunks.push(std::mem::take(&mut chunk));
                chunk_width = 0;
            }
            piece.push(ch);
            chunk_width += 1;
        }
        if !piece.is_empty() {
            chunk.push(Span::styled(piece, style));
        }
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}

fn span_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|span| span.content.chars().count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn joined(spans: &[Span<'static>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn wrap_value_breaks_on_spaces_within_width() {
        let lines = wrap_value(vec![Span::raw("date and time library")], 10);

        for line in &lines {
            assert!(
                joined(line).chars().count() <= 10,
                "line wider than 10: {:?}",
                joined(line)
            );
        }
        let rejoined = lines
            .iter()
            .map(|l| joined(l))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(rejoined, "date and time library");
    }

    #[test]
    fn wrap_value_hard_breaks_a_word_longer_than_width() {
        let url = "https://example.com/a/very/long/path";
        let lines = wrap_value(vec![Span::raw(url)], 10);

        assert!(lines.len() > 1);
        for line in &lines {
            assert!(joined(line).chars().count() <= 10);
        }
        // Hard-breaking must not drop or duplicate any characters.
        assert_eq!(lines.iter().map(|l| joined(l)).collect::<String>(), url);
    }

    #[test]
    fn labeled_lines_hang_indent_continuation_under_the_value_column() {
        let lines = labeled_lines(
            "Features:",
            Style::default(),
            vec![Span::raw("alpha beta gamma delta")],
            10,
            20,
        );

        assert!(lines.len() > 1);
        assert!(joined(&lines[0].spans).starts_with("Features: "));
        // Continuations begin with a full label-width indent so they align under the value.
        assert!(joined(&lines[1].spans).starts_with(&" ".repeat(10)));
    }
}
