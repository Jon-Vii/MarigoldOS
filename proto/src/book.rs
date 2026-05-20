pub const MAX_CATALOG_ITEMS: usize = 16;
pub const MAX_CHAPTERS: usize = 64;
pub const MAX_TITLE_BYTES: usize = 96;
pub const MAX_AUTHOR_BYTES: usize = 64;
pub const MAX_PATH_BYTES: usize = 160;
pub const MAX_CHAPTER_TITLE_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookSource {
    BuiltIn,
    MicroSd,
    SyncCache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverStatus {
    Unknown,
    Missing,
    Present,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookMeta<'a> {
    pub id: BookId,
    pub title: &'a str,
    pub author: &'a str,
    pub source_path: &'a str,
    pub byte_size: u32,
    pub source: BookSource,
    pub cover_status: CoverStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookProgress {
    pub spine_index: u16,
    pub rendered_screen_index: u32,
    pub whole_book_permille: u16,
}

impl BookProgress {
    pub const fn new(
        spine_index: u16,
        rendered_screen_index: u32,
        whole_book_permille: u16,
    ) -> Self {
        Self {
            spine_index,
            rendered_screen_index,
            whole_book_permille,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChapterMeta<'a> {
    pub title: &'a str,
    pub spine_index: u16,
    pub source_href: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Catalog<'a> {
    books: &'a [BookMeta<'a>],
}

impl<'a> Catalog<'a> {
    pub const fn new(books: &'a [BookMeta<'a>]) -> Self {
        Self { books }
    }

    pub const fn len(&self) -> usize {
        self.books.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.books.is_empty()
    }

    pub fn books(&self) -> &'a [BookMeta<'a>] {
        self.books
    }

    pub fn first(&self) -> Option<BookMeta<'a>> {
        self.books.first().copied()
    }

    pub fn by_id(&self, id: BookId) -> Option<BookMeta<'a>> {
        self.books.iter().copied().find(|book| book.id == id)
    }

    pub fn get(&self, index: usize) -> Option<BookMeta<'a>> {
        self.books.get(index).copied()
    }
}
