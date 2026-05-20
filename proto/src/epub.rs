use crate::book::{BookId, BookMeta, BookSource, ChapterMeta, CoverStatus};
use crate::text::{FontStyle, TextRole, TextRun};
use heapless::Vec;
use miniz_oxide::inflate::decompress_slice_iter_to_slice;

pub const MAX_SPINE_ITEMS: usize = 64;
pub const MAX_MANIFEST_ITEMS: usize = 96;
pub const MAX_ENTRY_NAME_BYTES: usize = 160;

pub trait ByteStream {
    type Error;

    fn read(&mut self, out: &mut [u8]) -> Result<usize, Self::Error>;
}

pub trait ReadAt {
    type Error;

    fn len(&mut self) -> Result<u32, Self::Error>;
    fn is_empty(&mut self) -> Result<bool, Self::Error> {
        Ok(self.len()? == 0)
    }
    fn read_at(&mut self, offset: u32, out: &mut [u8]) -> Result<usize, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Start(&'a str),
    End(&'a str),
    Text(&'a str),
}

pub struct XmlCursor<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> XmlCursor<'a> {
    pub const fn new(input: &'a str) -> Self {
        Self { input, cursor: 0 }
    }

    pub fn next_token(&mut self) -> Option<Token<'a>> {
        while self.cursor < self.input.len() {
            let rest = &self.input[self.cursor..];
            if let Some(after_lt) = rest.strip_prefix('<') {
                let end = after_lt.find('>')?;
                self.cursor += end + 2;
                let tag = after_lt[..end].trim();
                if let Some(name) = tag.strip_prefix('/') {
                    return Some(Token::End(name.trim()));
                }
                return Some(Token::Start(tag.split_whitespace().next().unwrap_or(tag)));
            }

            let end = rest.find('<').unwrap_or(rest.len());
            self.cursor += end;
            let text = rest[..end].trim();
            if !text.is_empty() {
                return Some(Token::Text(text));
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZipError {
    MissingEndOfCentralDirectory,
    BadCentralDirectory,
    BadLocalHeader,
    EntryNotFound,
    NameTooLong,
    UnsupportedCompression,
    OutputTooSmall,
    Inflate,
    Io,
    EntryBufferTooSmall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZipEntry<'a> {
    pub name: &'a str,
    pub compression_method: u16,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub local_header_offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedZipEntry {
    pub compression_method: u16,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub local_header_offset: u32,
}

pub struct ZipStream<R> {
    reader: R,
    central_offset: u32,
    entry_count: u16,
}

impl<R> ZipStream<R>
where
    R: ReadAt,
{
    pub fn new(mut reader: R, tail_scratch: &mut [u8]) -> Result<Self, ZipError> {
        let len = reader.len().map_err(|_| ZipError::Io)?;
        if len < 22 {
            return Err(ZipError::MissingEndOfCentralDirectory);
        }
        let tail_len = tail_scratch.len().min(len as usize);
        let tail_offset = len - tail_len as u32;
        read_exact_at(&mut reader, tail_offset, &mut tail_scratch[..tail_len])?;
        let eocd_in_tail =
            find_eocd(&tail_scratch[..tail_len]).ok_or(ZipError::MissingEndOfCentralDirectory)?;
        let eocd = eocd_in_tail;
        let entry_count = read_u16(tail_scratch, eocd + 10)?;
        let central_offset = read_u32(tail_scratch, eocd + 16)?;
        Ok(Self {
            reader,
            central_offset,
            entry_count,
        })
    }

    pub fn find_entry(
        &mut self,
        name: &str,
        header_scratch: &mut [u8; 46],
        name_scratch: &mut [u8],
    ) -> Result<OwnedZipEntry, ZipError> {
        let mut cursor = self.central_offset;
        for _ in 0..self.entry_count {
            read_exact_at(&mut self.reader, cursor, header_scratch)?;
            if read_u32(header_scratch, 0)? != 0x0201_4b50 {
                return Err(ZipError::BadCentralDirectory);
            }
            let compression_method = read_u16(header_scratch, 10)?;
            let compressed_size = read_u32(header_scratch, 20)?;
            let uncompressed_size = read_u32(header_scratch, 24)?;
            let name_len = read_u16(header_scratch, 28)? as usize;
            let extra_len = read_u16(header_scratch, 30)? as u32;
            let comment_len = read_u16(header_scratch, 32)? as u32;
            let local_header_offset = read_u32(header_scratch, 42)?;
            if name_len > name_scratch.len() {
                return Err(ZipError::EntryBufferTooSmall);
            }
            read_exact_at(&mut self.reader, cursor + 46, &mut name_scratch[..name_len])?;
            if core::str::from_utf8(&name_scratch[..name_len])
                .map(|entry_name| entry_name == name)
                .unwrap_or(false)
            {
                return Ok(OwnedZipEntry {
                    compression_method,
                    compressed_size,
                    uncompressed_size,
                    local_header_offset,
                });
            }
            cursor = cursor
                .checked_add(46 + name_len as u32 + extra_len + comment_len)
                .ok_or(ZipError::BadCentralDirectory)?;
        }
        Err(ZipError::EntryNotFound)
    }

    pub fn read_entry(
        &mut self,
        entry: OwnedZipEntry,
        compressed_scratch: &mut [u8],
        output: &mut [u8],
    ) -> Result<usize, ZipError> {
        if entry.compressed_size as usize > compressed_scratch.len() {
            return Err(ZipError::OutputTooSmall);
        }
        let payload_offset = self.entry_payload_offset(entry)?;
        let compressed = &mut compressed_scratch[..entry.compressed_size as usize];
        read_exact_at(&mut self.reader, payload_offset, compressed)?;
        match entry.compression_method {
            0 => {
                if output.len() < compressed.len() {
                    return Err(ZipError::OutputTooSmall);
                }
                output[..compressed.len()].copy_from_slice(compressed);
                Ok(compressed.len())
            }
            8 => decompress_slice_iter_to_slice(
                output,
                core::iter::once(&compressed[..]),
                false,
                true,
            )
            .map_err(|_| ZipError::Inflate),
            _ => Err(ZipError::UnsupportedCompression),
        }
    }

    fn entry_payload_offset(&mut self, entry: OwnedZipEntry) -> Result<u32, ZipError> {
        let mut header = [0u8; 30];
        read_exact_at(&mut self.reader, entry.local_header_offset, &mut header)?;
        if read_u32(&header, 0)? != 0x0403_4b50 {
            return Err(ZipError::BadLocalHeader);
        }
        let name_len = read_u16(&header, 26)? as u32;
        let extra_len = read_u16(&header, 28)? as u32;
        entry
            .local_header_offset
            .checked_add(30 + name_len + extra_len)
            .ok_or(ZipError::BadLocalHeader)
    }
}

pub struct ZipArchive<'a> {
    bytes: &'a [u8],
    central_offset: usize,
    entry_count: usize,
}

impl<'a> ZipArchive<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, ZipError> {
        let eocd = find_eocd(bytes).ok_or(ZipError::MissingEndOfCentralDirectory)?;
        if eocd + 22 > bytes.len() {
            return Err(ZipError::BadCentralDirectory);
        }
        let entry_count = read_u16(bytes, eocd + 10)? as usize;
        let central_size = read_u32(bytes, eocd + 12)? as usize;
        let central_offset = read_u32(bytes, eocd + 16)? as usize;
        if central_offset
            .checked_add(central_size)
            .filter(|end| *end <= bytes.len())
            .is_none()
        {
            return Err(ZipError::BadCentralDirectory);
        }
        Ok(Self {
            bytes,
            central_offset,
            entry_count,
        })
    }

    pub fn entries(&self) -> ZipEntries<'a> {
        ZipEntries {
            bytes: self.bytes,
            cursor: self.central_offset,
            remaining: self.entry_count,
        }
    }

    pub fn find(&self, name: &str) -> Result<ZipEntry<'a>, ZipError> {
        self.entries()
            .find(|entry| entry.map(|entry| entry.name == name).unwrap_or(false))
            .ok_or(ZipError::EntryNotFound)?
    }

    pub fn read_entry(&self, entry: ZipEntry<'a>, output: &mut [u8]) -> Result<usize, ZipError> {
        let compressed = self.entry_payload(entry)?;
        match entry.compression_method {
            0 => {
                if output.len() < compressed.len() {
                    return Err(ZipError::OutputTooSmall);
                }
                output[..compressed.len()].copy_from_slice(compressed);
                Ok(compressed.len())
            }
            8 => decompress_slice_iter_to_slice(output, core::iter::once(compressed), false, true)
                .map_err(|_| ZipError::Inflate),
            _ => Err(ZipError::UnsupportedCompression),
        }
    }

    fn entry_payload(&self, entry: ZipEntry<'a>) -> Result<&'a [u8], ZipError> {
        let offset = entry.local_header_offset as usize;
        if read_u32(self.bytes, offset)? != 0x0403_4b50 {
            return Err(ZipError::BadLocalHeader);
        }
        let name_len = read_u16(self.bytes, offset + 26)? as usize;
        let extra_len = read_u16(self.bytes, offset + 28)? as usize;
        let start = offset
            .checked_add(30)
            .and_then(|value| value.checked_add(name_len))
            .and_then(|value| value.checked_add(extra_len))
            .ok_or(ZipError::BadLocalHeader)?;
        let end = start
            .checked_add(entry.compressed_size as usize)
            .ok_or(ZipError::BadLocalHeader)?;
        self.bytes.get(start..end).ok_or(ZipError::BadLocalHeader)
    }
}

pub struct ZipEntries<'a> {
    bytes: &'a [u8],
    cursor: usize,
    remaining: usize,
}

impl<'a> Iterator for ZipEntries<'a> {
    type Item = Result<ZipEntry<'a>, ZipError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let result = parse_central_entry(self.bytes, self.cursor);
        if let Ok((entry, next_cursor)) = result {
            self.cursor = next_cursor;
            Some(Ok(entry))
        } else {
            self.remaining = 0;
            Some(result.map(|(entry, _)| entry))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpubError {
    Zip(ZipError),
    Utf8,
    MissingContainer,
    MissingOpfPath,
    MissingOpf,
    TooManyManifestItems,
    TooManySpineItems,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XhtmlError {
    TooManyRuns,
}

impl From<ZipError> for EpubError {
    fn from(value: ZipError) -> Self {
        Self::Zip(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestItem<'a> {
    pub id: &'a str,
    pub href: &'a str,
    pub media_type: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpineItem<'a> {
    pub idref: &'a str,
    pub href: &'a str,
}

pub struct EpubPackage<'a> {
    pub meta: BookMeta<'a>,
    pub opf_path: &'a str,
    pub manifest: Vec<ManifestItem<'a>, MAX_MANIFEST_ITEMS>,
    pub spine: Vec<SpineItem<'a>, MAX_SPINE_ITEMS>,
}

impl<'a> EpubPackage<'a> {
    pub fn chapters(&self, output: &mut Vec<ChapterMeta<'a>, MAX_SPINE_ITEMS>) {
        output.clear();
        for (index, spine) in self.spine.iter().enumerate() {
            let title = spine.href.rsplit('/').next().unwrap_or(spine.href);
            let _ = output.push(ChapterMeta {
                title,
                spine_index: index as u16,
                source_href: spine.href,
            });
        }
    }
}

pub fn load_epub_package<'a>(
    epub_bytes: &'a [u8],
    container_scratch: &'a mut [u8],
    opf_scratch: &'a mut [u8],
    book_id: BookId,
    source_path: &'a str,
) -> Result<EpubPackage<'a>, EpubError> {
    let zip = ZipArchive::new(epub_bytes)?;
    let container = zip.find("META-INF/container.xml")?;
    let container_len = zip.read_entry(container, container_scratch)?;
    let container_xml =
        core::str::from_utf8(&container_scratch[..container_len]).map_err(|_| EpubError::Utf8)?;
    let opf_path =
        find_attr_value(container_xml, "rootfile", "full-path").ok_or(EpubError::MissingOpfPath)?;

    let opf_entry = zip.find(opf_path).map_err(|_| EpubError::MissingOpf)?;
    let opf_len = zip.read_entry(opf_entry, opf_scratch)?;
    let opf_xml = core::str::from_utf8(&opf_scratch[..opf_len]).map_err(|_| EpubError::Utf8)?;
    parse_opf(
        opf_xml,
        book_id,
        source_path,
        epub_bytes.len() as u32,
        opf_path,
    )
}

pub fn parse_opf<'a>(
    opf_xml: &'a str,
    book_id: BookId,
    source_path: &'a str,
    byte_size: u32,
    opf_path: &'a str,
) -> Result<EpubPackage<'a>, EpubError> {
    let title = element_text(opf_xml, "dc:title")
        .or_else(|| element_text(opf_xml, "title"))
        .unwrap_or("Untitled");
    let author = element_text(opf_xml, "dc:creator")
        .or_else(|| element_text(opf_xml, "creator"))
        .unwrap_or("Unknown Author");

    let mut manifest = Vec::new();
    let mut cursor = 0;
    while let Some((tag, next)) = next_start_tag(opf_xml, "item", cursor) {
        cursor = next;
        let Some(id) = attr_value(tag, "id") else {
            continue;
        };
        let Some(href) = attr_value(tag, "href") else {
            continue;
        };
        let media_type = attr_value(tag, "media-type").unwrap_or("");
        manifest
            .push(ManifestItem {
                id,
                href,
                media_type,
            })
            .map_err(|_| EpubError::TooManyManifestItems)?;
    }

    let mut spine = Vec::new();
    cursor = 0;
    while let Some((tag, next)) = next_start_tag(opf_xml, "itemref", cursor) {
        cursor = next;
        let Some(idref) = attr_value(tag, "idref") else {
            continue;
        };
        let href = manifest
            .iter()
            .find(|item| item.id == idref)
            .map(|item| item.href)
            .unwrap_or("");
        spine
            .push(SpineItem { idref, href })
            .map_err(|_| EpubError::TooManySpineItems)?;
    }

    Ok(EpubPackage {
        meta: BookMeta {
            id: book_id,
            title,
            author,
            source_path,
            byte_size,
            source: BookSource::MicroSd,
            cover_status: cover_status(&manifest),
        },
        opf_path,
        manifest,
        spine,
    })
}

pub fn xhtml_text_runs<'a>(
    xhtml: &'a str,
    output: &mut Vec<TextRun<'a>, 256>,
) -> Result<(), XhtmlError> {
    output.clear();
    let mut cursor = XmlCursor::new(xhtml);
    let mut role = TextRole::Body;
    let mut bold_depth = 0u8;
    let mut italic_depth = 0u8;

    while let Some(token) = cursor.next_token() {
        match token {
            Token::Start("h1") => {
                role = TextRole::Heading1;
                bold_depth = bold_depth.saturating_add(1);
            }
            Token::Start("h2") => {
                role = TextRole::Heading2;
                bold_depth = bold_depth.saturating_add(1);
            }
            Token::Start("h3") => {
                role = TextRole::Heading3;
                bold_depth = bold_depth.saturating_add(1);
            }
            Token::Start("blockquote") => {
                role = TextRole::BlockQuote;
                italic_depth = italic_depth.saturating_add(1);
            }
            Token::Start("strong") | Token::Start("b") => {
                bold_depth = bold_depth.saturating_add(1);
            }
            Token::Start("em") | Token::Start("i") => {
                italic_depth = italic_depth.saturating_add(1);
            }
            Token::End("h1") | Token::End("h2") | Token::End("h3") => {
                role = TextRole::Body;
                bold_depth = bold_depth.saturating_sub(1);
            }
            Token::End("blockquote") => {
                role = TextRole::Body;
                italic_depth = italic_depth.saturating_sub(1);
            }
            Token::End("strong") | Token::End("b") => {
                bold_depth = bold_depth.saturating_sub(1);
            }
            Token::End("em") | Token::End("i") => {
                italic_depth = italic_depth.saturating_sub(1);
            }
            Token::Text(text) => {
                output
                    .push(TextRun::new(
                        text,
                        role,
                        style_for(bold_depth, italic_depth),
                    ))
                    .map_err(|_| XhtmlError::TooManyRuns)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn parse_central_entry(bytes: &[u8], cursor: usize) -> Result<(ZipEntry<'_>, usize), ZipError> {
    if read_u32(bytes, cursor)? != 0x0201_4b50 {
        return Err(ZipError::BadCentralDirectory);
    }
    let compression_method = read_u16(bytes, cursor + 10)?;
    let compressed_size = read_u32(bytes, cursor + 20)?;
    let uncompressed_size = read_u32(bytes, cursor + 24)?;
    let name_len = read_u16(bytes, cursor + 28)? as usize;
    let extra_len = read_u16(bytes, cursor + 30)? as usize;
    let comment_len = read_u16(bytes, cursor + 32)? as usize;
    let local_header_offset = read_u32(bytes, cursor + 42)?;
    let name_start = cursor + 46;
    let name_end = name_start
        .checked_add(name_len)
        .ok_or(ZipError::BadCentralDirectory)?;
    let next = name_end
        .checked_add(extra_len)
        .and_then(|value| value.checked_add(comment_len))
        .ok_or(ZipError::BadCentralDirectory)?;
    let name_bytes = bytes
        .get(name_start..name_end)
        .ok_or(ZipError::BadCentralDirectory)?;
    if name_len > MAX_ENTRY_NAME_BYTES {
        return Err(ZipError::NameTooLong);
    }
    let name = core::str::from_utf8(name_bytes).map_err(|_| ZipError::BadCentralDirectory)?;
    Ok((
        ZipEntry {
            name,
            compression_method,
            compressed_size,
            uncompressed_size,
            local_header_offset,
        },
        next,
    ))
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }
    let mut cursor = bytes.len() - 22;
    loop {
        if bytes.get(cursor..cursor + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]) {
            return Some(cursor);
        }
        if cursor == 0 {
            return None;
        }
        cursor -= 1;
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ZipError> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or(ZipError::BadCentralDirectory)?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ZipError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or(ZipError::BadCentralDirectory)?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_exact_at<R>(reader: &mut R, offset: u32, out: &mut [u8]) -> Result<(), ZipError>
where
    R: ReadAt,
{
    let mut filled = 0;
    while filled < out.len() {
        let count = reader
            .read_at(offset + filled as u32, &mut out[filled..])
            .map_err(|_| ZipError::Io)?;
        if count == 0 {
            return Err(ZipError::Io);
        }
        filled += count;
    }
    Ok(())
}

fn next_start_tag<'a>(xml: &'a str, name: &str, from: usize) -> Option<(&'a str, usize)> {
    let mut cursor = from;
    while let Some(relative) = xml[cursor..].find('<') {
        let start = cursor + relative + 1;
        let end = start + xml[start..].find('>')?;
        let tag = xml[start..end].trim();
        let tag_name = tag.split_whitespace().next().unwrap_or(tag);
        if tag_name == name {
            return Some((tag, end + 1));
        }
        cursor = end + 1;
    }
    None
}

fn find_attr_value<'a>(xml: &'a str, tag_name: &str, attr: &str) -> Option<&'a str> {
    let mut cursor = 0;
    while let Some((tag, next)) = next_start_tag(xml, tag_name, cursor) {
        if let Some(value) = attr_value(tag, attr) {
            return Some(value);
        }
        cursor = next;
    }
    None
}

fn attr_value<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = name;
    let mut rest = tag;
    loop {
        let position = rest.find(needle)?;
        let after_name = &rest[position + needle.len()..];
        let after_eq = after_name.trim_start().strip_prefix('=')?.trim_start();
        let quote = after_eq.as_bytes().first().copied()?;
        if quote != b'\'' && quote != b'"' {
            rest = after_name;
            continue;
        }
        let value = &after_eq[1..];
        let end = value.find(quote as char)?;
        return Some(&value[..end]);
    }
}

fn element_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = {
        let mut cursor = 0;
        loop {
            let (candidate, next) = next_start_tag(xml, tag, cursor)?;
            let end = next;
            if candidate.split_whitespace().next().unwrap_or(candidate) == tag {
                break end;
            }
            cursor = next;
        }
    };
    let close_tag = close_tag(tag);
    let close = xml[open..].find(close_tag.as_str())? + open;
    let value = xml[open..close].trim();
    (!value.is_empty()).then_some(value)
}

fn close_tag(tag: &str) -> heapless::String<40> {
    let mut close = heapless::String::new();
    let _ = close.push_str("</");
    let _ = close.push_str(tag);
    let _ = close.push('>');
    close
}

fn cover_status(manifest: &[ManifestItem<'_>]) -> CoverStatus {
    if manifest.iter().any(|item| {
        item.id == "cover" || item.href.contains("cover") || item.media_type.starts_with("image/")
    }) {
        CoverStatus::Present
    } else {
        CoverStatus::Missing
    }
}

fn style_for(bold_depth: u8, italic_depth: u8) -> FontStyle {
    match (bold_depth > 0, italic_depth > 0) {
        (true, true) => FontStyle::BoldItalic,
        (true, false) => FontStyle::Bold,
        (false, true) => FontStyle::Italic,
        (false, false) => FontStyle::Regular,
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec as StdVec;

    struct SliceReader<'a> {
        bytes: &'a [u8],
    }

    impl ReadAt for SliceReader<'_> {
        type Error = ();

        fn len(&mut self) -> Result<u32, Self::Error> {
            Ok(self.bytes.len() as u32)
        }

        fn read_at(&mut self, offset: u32, out: &mut [u8]) -> Result<usize, Self::Error> {
            let offset = offset as usize;
            let Some(rest) = self.bytes.get(offset..) else {
                return Ok(0);
            };
            let count = out.len().min(rest.len());
            out[..count].copy_from_slice(&rest[..count]);
            Ok(count)
        }
    }

    #[test]
    fn parses_nested_opf_path_and_spine() {
        let opf = r#"
            <package>
              <metadata>
                <dc:title>Flowers for Algernon</dc:title>
                <dc:creator>Daniel Keyes</dc:creator>
              </metadata>
              <manifest>
                <item id="chap1" href="text/ch1.xhtml" media-type="application/xhtml+xml"/>
                <item id="cover" href="images/cover.jpg" media-type="image/jpeg"/>
              </manifest>
              <spine><itemref idref="chap1"/></spine>
            </package>
        "#;

        let package = parse_opf(
            opf,
            BookId(7),
            "/books/flowers.epub",
            1234,
            "OPS/package.opf",
        )
        .expect("opf parses");

        assert_eq!(package.meta.title, "Flowers for Algernon");
        assert_eq!(package.meta.author, "Daniel Keyes");
        assert_eq!(package.meta.cover_status, CoverStatus::Present);
        assert_eq!(package.spine.len(), 1);
        assert_eq!(package.spine[0].href, "text/ch1.xhtml");
    }

    #[test]
    fn xhtml_emits_styled_runs() {
        let xhtml = "<body><h1>Chapter</h1><p>Hello <em>soft</em> <strong>bold</strong></p></body>";
        let mut runs = heapless::Vec::<TextRun<'_>, 256>::new();

        xhtml_text_runs(xhtml, &mut runs).expect("runs fit");

        assert_eq!(
            runs[0],
            TextRun::new("Chapter", TextRole::Heading1, FontStyle::Bold)
        );
        assert_eq!(
            runs[1],
            TextRun::new("Hello", TextRole::Body, FontStyle::Regular)
        );
        assert_eq!(
            runs[2],
            TextRun::new("soft", TextRole::Body, FontStyle::Italic)
        );
        assert_eq!(
            runs[3],
            TextRun::new("bold", TextRole::Body, FontStyle::Bold)
        );
    }

    #[test]
    fn zip_rejects_missing_entry() {
        let zip_bytes = stored_zip(&[("hello.txt", b"hi".as_slice())]);
        let archive = ZipArchive::new(&zip_bytes).expect("zip parses");

        assert_eq!(archive.find("missing.txt"), Err(ZipError::EntryNotFound));
    }

    #[test]
    fn zip_rejects_malformed_central_directory() {
        assert_eq!(
            ZipArchive::new(b"not a zip file").err(),
            Some(ZipError::MissingEndOfCentralDirectory)
        );
    }

    #[test]
    fn zip_reads_stored_entry() {
        let zip_bytes = stored_zip(&[("META-INF/container.xml", b"<container/>".as_slice())]);
        let archive = ZipArchive::new(&zip_bytes).expect("zip parses");
        let entry = archive
            .find("META-INF/container.xml")
            .expect("entry exists");
        let mut output = [0u8; 32];

        let len = archive.read_entry(entry, &mut output).expect("stored read");

        assert_eq!(&output[..len], b"<container/>");
    }

    #[test]
    fn zip_stream_reads_stored_entry_by_offset() {
        let zip_bytes = stored_zip(&[("OPS/package.opf", b"<package/>".as_slice())]);
        let mut stream = ZipStream::new(SliceReader { bytes: &zip_bytes }, &mut [0u8; 512])
            .expect("stream zip parses");
        let entry = stream
            .find_entry("OPS/package.opf", &mut [0u8; 46], &mut [0u8; 64])
            .expect("entry exists");
        let mut compressed = [0u8; 64];
        let mut output = [0u8; 64];

        let len = stream
            .read_entry(entry, &mut compressed, &mut output)
            .expect("entry read");

        assert_eq!(&output[..len], b"<package/>");
    }

    fn stored_zip(files: &[(&str, &[u8])]) -> StdVec<u8> {
        let mut bytes = StdVec::new();
        let mut central = StdVec::new();
        let mut offsets = StdVec::new();

        for (name, data) in files {
            offsets.push(bytes.len() as u32);
            push_u32(&mut bytes, 0x0403_4b50);
            push_u16(&mut bytes, 20);
            push_u16(&mut bytes, 0);
            push_u16(&mut bytes, 0);
            push_u16(&mut bytes, 0);
            push_u16(&mut bytes, 0);
            push_u32(&mut bytes, 0);
            push_u32(&mut bytes, data.len() as u32);
            push_u32(&mut bytes, data.len() as u32);
            push_u16(&mut bytes, name.len() as u16);
            push_u16(&mut bytes, 0);
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(data);
        }

        for ((name, data), offset) in files.iter().zip(offsets.iter()) {
            push_u32(&mut central, 0x0201_4b50);
            push_u16(&mut central, 20);
            push_u16(&mut central, 20);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u32(&mut central, 0);
            push_u32(&mut central, data.len() as u32);
            push_u32(&mut central, data.len() as u32);
            push_u16(&mut central, name.len() as u16);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u16(&mut central, 0);
            push_u32(&mut central, 0);
            push_u32(&mut central, *offset);
            central.extend_from_slice(name.as_bytes());
        }

        let central_offset = bytes.len() as u32;
        let central_size = central.len() as u32;
        bytes.extend_from_slice(&central);
        push_u32(&mut bytes, 0x0605_4b50);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, files.len() as u16);
        push_u16(&mut bytes, files.len() as u16);
        push_u32(&mut bytes, central_size);
        push_u32(&mut bytes, central_offset);
        push_u16(&mut bytes, 0);
        bytes
    }

    fn push_u16(bytes: &mut StdVec<u8>, value: u16) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(bytes: &mut StdVec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
