use heapless::String;

pub const MAX_CATALOG_ITEMS: usize = 8;
pub const MAX_CHUNK_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    pub id: u32,
    pub title: String<48>,
    pub author: String<32>,
    pub bytes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Catalog,
    Download { book_id: u32, offset: u32 },
    Ack { book_id: u32, offset: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response<'a> {
    Catalog {
        books: &'a [Book],
    },
    Chunk {
        book_id: u32,
        offset: u32,
        data: &'a [u8],
        eof: bool,
    },
    Error {
        code: u8,
    },
}
