use proto::book::{BookId, BookMeta, BookSource, ChapterMeta, CoverStatus};

pub const ACTIVE_BOOK_ID: BookId = BookId(1);

pub const BOOKS: [BookMeta<'static>; 1] = [BookMeta {
    id: ACTIVE_BOOK_ID,
    title: "Flowers for Algernon",
    author: "Daniel Keyes",
    source_path: "/books/flowers-for-algernon.epub",
    byte_size: 0,
    source: BookSource::BuiltIn,
    cover_status: CoverStatus::Missing,
}];

pub const CHAPTERS: [ChapterMeta<'static>; 4] = [
    ChapterMeta {
        title: "Bring Up",
        spine_index: 0,
        source_href: "demo/bring-up.xhtml",
    },
    ChapterMeta {
        title: "Architecture",
        spine_index: 1,
        source_href: "demo/architecture.xhtml",
    },
    ChapterMeta {
        title: "Power",
        spine_index: 2,
        source_href: "demo/power.xhtml",
    },
    ChapterMeta {
        title: "Next Phase",
        spine_index: 3,
        source_href: "demo/next-phase.xhtml",
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderLineStyle {
    Heading,
    Body,
    Italic,
    Bold,
    Quote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReaderLine {
    pub text: &'static str,
    pub style: ReaderLineStyle,
    pub gap_after: u8,
}

impl ReaderLine {
    pub const fn new(text: &'static str, style: ReaderLineStyle, gap_after: u8) -> Self {
        Self {
            text,
            style,
            gap_after,
        }
    }
}

pub const READER_PAGES: [&[ReaderLine]; 4] = [
    &[
        ReaderLine::new("A quiet page, finally", ReaderLineStyle::Heading, 18),
        ReaderLine::new(
            "This is the first proper Literata reading proof on the X4. It uses uppercase, lowercase, punctuation, and enough ordinary sentence rhythm to show whether the page breathes.",
            ReaderLineStyle::Body,
            14,
        ),
        ReaderLine::new(
            "Italic text should feel softer, not like a debug overlay.",
            ReaderLineStyle::Italic,
            10,
        ),
        ReaderLine::new(
            "Bold text marks headings, chapter names, and emphasis.",
            ReaderLineStyle::Bold,
            14,
        ),
        ReaderLine::new(
            "The margins are intentionally narrow because the device already gives the page a physical white border.",
            ReaderLineStyle::Body,
            0,
        ),
    ],
    &[
        ReaderLine::new("Typography model", ReaderLineStyle::Heading, 18),
        ReaderLine::new(
            "EPUB text becomes styled runs before it reaches the panel. The renderer can choose Regular, Italic, Bold, or Bold Italic without changing the app state or reader controls.",
            ReaderLineStyle::Body,
            14,
        ),
        ReaderLine::new(
            "Block quotes will get indentation and a quieter italic voice.",
            ReaderLineStyle::Quote,
            10,
        ),
        ReaderLine::new(
            "Bookerly stays user-provided later; Literata ships built in.",
            ReaderLineStyle::Body,
            0,
        ),
    ],
    &[
        ReaderLine::new("Pagination", ReaderLineStyle::Heading, 18),
        ReaderLine::new(
            "A reader should never build a giant layout tree just to turn one page. The firmware keeps cursors and bounded runs, then paints exactly the screen it needs.",
            ReaderLineStyle::Body,
            14,
        ),
        ReaderLine::new(
            "Async input is coalesced while the display refreshes, so the CPU can go quiet instead of chasing a backlog of stale pages.",
            ReaderLineStyle::Body,
            0,
        ),
    ],
    &[
        ReaderLine::new("Storage", ReaderLineStyle::Heading, 18),
        ReaderLine::new(
            "The next hardware step is the microSD reader path.",
            ReaderLineStyle::Body,
            8,
        ),
        ReaderLine::new(
            "Files will scan /books first, then the card root, and parse real EPUB metadata for title and author.",
            ReaderLineStyle::Body,
            14,
        ),
        ReaderLine::new(
            "Progress belongs beside the books: current path, chapter, screen cursor, orientation, and refresh policy.",
            ReaderLineStyle::Body,
            0,
        ),
    ],
];

pub fn active_book(book_id: u32) -> BookMeta<'static> {
    BOOKS
        .iter()
        .copied()
        .find(|book| book.id.0 == book_id)
        .unwrap_or(BOOKS[0])
}

pub fn book_at(index: usize) -> Option<BookMeta<'static>> {
    BOOKS.get(index).copied()
}

pub const fn book_count() -> u8 {
    BOOKS.len() as u8
}

pub fn chapter_at(index: usize) -> Option<ChapterMeta<'static>> {
    CHAPTERS.get(index).copied()
}

pub const fn chapter_count() -> u8 {
    CHAPTERS.len() as u8
}
