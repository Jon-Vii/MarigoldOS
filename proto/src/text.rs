#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFamily {
    Literata,
    BookerlyUser,
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    Regular,
    Italic,
    Bold,
    BoldItalic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    Body,
    Heading1,
    Heading2,
    Heading3,
    BlockQuote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRun<'a> {
    pub text: &'a str,
    pub role: TextRole,
    pub style: FontStyle,
}

impl<'a> TextRun<'a> {
    pub const fn new(text: &'a str, role: TextRole, style: FontStyle) -> Self {
        Self { text, role, style }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextCursor {
    pub run_index: u16,
    pub byte_offset: u16,
    pub screen_index: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageLayout {
    pub columns: u16,
    pub lines: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageBreak {
    pub start: TextCursor,
    pub end: TextCursor,
    pub full: bool,
}

pub fn paginate_screen(runs: &[TextRun<'_>], start: TextCursor, layout: PageLayout) -> PageBreak {
    let mut cursor = start;
    let mut line = 0u16;
    let mut column = 0u16;
    let mut consumed_any = false;

    while (cursor.run_index as usize) < runs.len() && line < layout.lines {
        let run = runs[cursor.run_index as usize];
        let bytes = run.text.as_bytes();
        let mut offset = cursor.byte_offset as usize;

        while offset < bytes.len() && line < layout.lines {
            let byte = bytes[offset];
            if byte == b'\n' {
                line = line.saturating_add(1);
                column = 0;
                offset += 1;
                consumed_any = true;
                continue;
            }

            if column >= layout.columns {
                line = line.saturating_add(1);
                column = 0;
                if line >= layout.lines {
                    break;
                }
            }

            column = column.saturating_add(width_for(run.role, byte));
            offset += 1;
            consumed_any = true;
        }

        cursor.byte_offset = offset as u16;
        if offset >= bytes.len() {
            cursor.run_index = cursor.run_index.saturating_add(1);
            cursor.byte_offset = 0;
            if paragraph_break_after(run.role) && column > 0 {
                line = line.saturating_add(1);
                column = 0;
            }
        }
    }

    PageBreak {
        start,
        end: TextCursor {
            screen_index: start.screen_index.saturating_add(consumed_any as u32),
            ..cursor
        },
        full: line >= layout.lines,
    }
}

fn width_for(role: TextRole, byte: u8) -> u16 {
    if byte == b' ' {
        1
    } else {
        match role {
            TextRole::Heading1 => 2,
            TextRole::Heading2 => 2,
            TextRole::Heading3 | TextRole::Body | TextRole::BlockQuote => 1,
        }
    }
}

fn paragraph_break_after(role: TextRole) -> bool {
    matches!(
        role,
        TextRole::Body
            | TextRole::Heading1
            | TextRole::Heading2
            | TextRole::Heading3
            | TextRole::BlockQuote
    )
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_is_deterministic_for_fixed_layout() {
        let runs = [
            TextRun::new("Heading", TextRole::Heading1, FontStyle::Bold),
            TextRun::new(
                "one two three four five",
                TextRole::Body,
                FontStyle::Regular,
            ),
        ];
        let start = TextCursor {
            run_index: 0,
            byte_offset: 0,
            screen_index: 0,
        };

        let first = paginate_screen(
            &runs,
            start,
            PageLayout {
                columns: 10,
                lines: 3,
            },
        );
        let second = paginate_screen(
            &runs,
            start,
            PageLayout {
                columns: 10,
                lines: 3,
            },
        );

        assert_eq!(first, second);
        assert!(first.end.run_index > 0);
    }
}
