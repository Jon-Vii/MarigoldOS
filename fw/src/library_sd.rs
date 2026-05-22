use crate::display_flush::Epd;
use crate::reader_store::{LibraryScanStatus, ReaderStore};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{Operation, SpiBus as BlockingSpiBus, SpiDevice};
use embedded_sdmmc::{
    Directory, File, LfnBuffer, Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager,
};
use esp_hal::gpio::Output;
use esp_hal::prelude::*;
use heapless::String;

pub(crate) struct StaticTime;

const CATALOG_ROOT_DIR: &str = "XTEINK";
const CATALOG_FILE: &str = "CATALOG.BIN";
const CATALOG_MAGIC: &[u8; 4] = b"X4CT";
const CATALOG_VERSION: u8 = 1;
const CATALOG_HEADER_BYTES: usize = 8;
const CATALOG_RECORD_BYTES: usize = 92;

impl TimeSource for StaticTime {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 56,
            zero_indexed_month: 4,
            zero_indexed_day: 19,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub(crate) struct SdSpiDevice<'a, SPI, CS> {
    pub(crate) spi: &'a mut SPI,
    pub(crate) cs: &'a mut CS,
    pub(crate) delay: esp_hal::delay::Delay,
}

impl<SPI, CS> embedded_hal::spi::ErrorType for SdSpiDevice<'_, SPI, CS>
where
    SPI: embedded_hal::spi::ErrorType,
{
    type Error = SPI::Error;
}

impl<SPI, CS> SpiDevice for SdSpiDevice<'_, SPI, CS>
where
    SPI: BlockingSpiBus<u8>,
    CS: OutputPin,
{
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let _ = self.cs.set_low();
        let mut result = Ok(());

        for operation in operations {
            result = match operation {
                Operation::Read(buffer) => self.spi.read(buffer),
                Operation::Write(buffer) => self.spi.write(buffer),
                Operation::Transfer(read, write) => self.spi.transfer(read, write),
                Operation::TransferInPlace(buffer) => self.spi.transfer_in_place(buffer),
                Operation::DelayNs(ns) => {
                    self.delay.delay_ns(*ns);
                    Ok(())
                }
            };

            if result.is_err() {
                break;
            }
        }

        let _ = self.spi.flush();
        let _ = self.cs.set_high();
        result
    }
}

pub(crate) fn scan_books(epd: &mut Epd, sd_cs: &mut Output<'static>, library: &mut ReaderStore) {
    esp_println::println!("sd: scan start");
    library.status = LibraryScanStatus::Scanning;
    epd.deselect_display();
    sd_cs.set_high();
    epd.spi_mut().change_bus_frequency(400_u32.kHz());

    let startup_clocks = [0xFF; 10];
    if BlockingSpiBus::write(epd.spi_mut(), &startup_clocks).is_err() {
        esp_println::println!("sd: startup clocks failed");
        epd.spi_mut().change_bus_frequency(40_u32.MHz());
        library.status = LibraryScanStatus::Error;
        return;
    }

    let status = 'scan: {
        let spi = SdSpiDevice {
            spi: epd.spi_mut(),
            cs: sd_cs,
            delay: esp_hal::delay::Delay::new(),
        };
        let card = SdCard::new(spi, esp_hal::delay::Delay::new());
        esp_println::println!("sd: card init begin");
        match card.num_bytes() {
            Ok(bytes) => esp_println::println!("sd: card size {} bytes", bytes),
            Err(err) => {
                esp_println::println!("sd: card init failed: {:?}", err);
                break 'scan LibraryScanStatus::Error;
            }
        }

        card.spi(|device| device.spi.change_bus_frequency(8_u32.MHz()));
        esp_println::println!("sd: open volume");
        let volume_mgr: VolumeManager<_, _, 4, 4, 1> = VolumeManager::new(card, StaticTime);
        let volume = match volume_mgr.open_volume(VolumeIdx(0)) {
            Ok(volume) => volume,
            Err(err) => {
                esp_println::println!("sd: open volume failed: {:?}", err);
                break 'scan LibraryScanStatus::Error;
            }
        };
        esp_println::println!("sd: open root");
        let root = match volume.open_root_dir() {
            Ok(root) => root,
            Err(err) => {
                esp_println::println!("sd: open root failed: {:?}", err);
                break 'scan LibraryScanStatus::Error;
            }
        };

        library.clear_catalog();
        library.status = LibraryScanStatus::Scanning;
        if let Ok(books) = root.open_dir("BOOKS") {
            collect_epubs(&books, "/books/", true, library);
        }
        if library.count == 0 {
            collect_epubs(&root, "/", false, library);
        }

        if library.count == 0 {
            LibraryScanStatus::Empty
        } else {
            let _ = write_catalog_cache(&root, library);
            LibraryScanStatus::Ready
        }
    };
    epd.spi_mut().change_bus_frequency(40_u32.MHz());
    library.status = if status == LibraryScanStatus::Error && library.count > 0 {
        LibraryScanStatus::Ready
    } else {
        status
    };
    esp_println::println!("sd: scan complete, {} epub(s)", library.count);
}

pub(crate) fn load_catalog_cache(
    epd: &mut Epd,
    sd_cs: &mut Output<'static>,
    library: &mut ReaderStore,
) -> bool {
    esp_println::println!("sd: catalog cache load start");
    epd.deselect_display();
    sd_cs.set_high();
    epd.spi_mut().change_bus_frequency(400_u32.kHz());

    let startup_clocks = [0xFF; 10];
    if BlockingSpiBus::write(epd.spi_mut(), &startup_clocks).is_err() {
        epd.spi_mut().change_bus_frequency(40_u32.MHz());
        return false;
    }

    let loaded = 'load: {
        let spi = SdSpiDevice {
            spi: epd.spi_mut(),
            cs: sd_cs,
            delay: esp_hal::delay::Delay::new(),
        };
        let card = SdCard::new(spi, esp_hal::delay::Delay::new());
        if card.num_bytes().is_err() {
            break 'load false;
        }
        card.spi(|device| device.spi.change_bus_frequency(8_u32.MHz()));
        let volume_mgr: VolumeManager<_, _, 4, 4, 1> = VolumeManager::new(card, StaticTime);
        let Ok(volume) = volume_mgr.open_volume(VolumeIdx(0)) else {
            break 'load false;
        };
        let Ok(root) = volume.open_root_dir() else {
            break 'load false;
        };
        read_catalog_cache(&root, library).is_ok()
    };
    epd.spi_mut().change_bus_frequency(40_u32.MHz());
    if loaded {
        esp_println::println!("sd: catalog cache loaded {} epub(s)", library.count);
    } else {
        esp_println::println!("sd: catalog cache unavailable");
    }
    loaded
}

fn write_catalog_cache<
    D,
    T,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
>(
    root: &Directory<'_, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    library: &ReaderStore,
) -> Result<(), ()>
where
    D: embedded_sdmmc::BlockDevice,
    T: TimeSource,
{
    let xteink = open_or_make_dir(root, CATALOG_ROOT_DIR)?;
    let file = xteink
        .open_file_in_dir(CATALOG_FILE, Mode::ReadWriteCreateOrTruncate)
        .map_err(|_| ())?;
    let mut header = [0u8; CATALOG_HEADER_BYTES];
    header[..4].copy_from_slice(CATALOG_MAGIC);
    header[4] = CATALOG_VERSION;
    header[5] = library.count.min(u8::MAX as usize) as u8;
    file.write(&header).map_err(|_| ())?;
    let mut record = [0u8; CATALOG_RECORD_BYTES];
    for entry in library.entries.iter().take(library.count) {
        record.fill(0);
        record[0] = entry.in_books_dir as u8;
        record[4..8].copy_from_slice(&entry.byte_size.to_le_bytes());
        record[8..12].copy_from_slice(&entry.source_hash.to_le_bytes());
        copy_fixed(entry.display_name.as_bytes(), &mut record[12..76]);
        copy_fixed(entry.open_name.as_bytes(), &mut record[76..92]);
        file.write(&record).map_err(|_| ())?;
    }
    Ok(())
}

fn read_catalog_cache<
    D,
    T,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
>(
    root: &Directory<'_, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    library: &mut ReaderStore,
) -> Result<(), ()>
where
    D: embedded_sdmmc::BlockDevice,
    T: TimeSource,
{
    let xteink = root.open_dir(CATALOG_ROOT_DIR).map_err(|_| ())?;
    let file = xteink
        .open_file_in_dir(CATALOG_FILE, Mode::ReadOnly)
        .map_err(|_| ())?;
    let mut header = [0u8; CATALOG_HEADER_BYTES];
    read_exact_file(&file, &mut header)?;
    if &header[..4] != CATALOG_MAGIC || header[4] != CATALOG_VERSION {
        return Err(());
    }
    let count = header[5] as usize;
    library.clear_catalog();
    let mut record = [0u8; CATALOG_RECORD_BYTES];
    for _ in 0..count.min(crate::reader_store::MAX_LIBRARY_BOOKS) {
        read_exact_file(&file, &mut record)?;
        let display_name = fixed_str(&record[12..76]);
        let open_name = fixed_str(&record[76..92]);
        if display_name.is_empty() || open_name.is_empty() {
            continue;
        }
        library.push(
            display_name,
            open_name,
            record[0] != 0,
            u32::from_le_bytes([record[4], record[5], record[6], record[7]]),
        );
        if let Some(entry) = library.entries.get_mut(library.count.saturating_sub(1)) {
            entry.source_hash = u32::from_le_bytes([record[8], record[9], record[10], record[11]]);
        }
    }
    library.status = if library.count == 0 {
        LibraryScanStatus::Empty
    } else {
        LibraryScanStatus::Ready
    };
    Ok(())
}

fn open_or_make_dir<
    'a,
    D,
    T,
    const MAX_DIRS: usize,
    const MAX_FILES: usize,
    const MAX_VOLUMES: usize,
>(
    parent: &'a Directory<'_, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    name: &str,
) -> Result<Directory<'a, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>, ()>
where
    D: embedded_sdmmc::BlockDevice,
    T: TimeSource,
{
    match parent.open_dir(name) {
        Ok(dir) => Ok(dir),
        Err(_) => {
            let _ = parent.make_dir_in_dir(name);
            parent.open_dir(name).map_err(|_| ())
        }
    }
}

fn read_exact_file<D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>(
    file: &File<'_, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    mut out: &mut [u8],
) -> Result<(), ()>
where
    D: embedded_sdmmc::BlockDevice,
    T: TimeSource,
{
    while !out.is_empty() {
        let read = file.read(out).map_err(|_| ())?;
        if read == 0 {
            return Err(());
        }
        let tmp = out;
        out = &mut tmp[read..];
    }
    Ok(())
}

fn copy_fixed(src: &[u8], dst: &mut [u8]) {
    let len = src.len().min(dst.len());
    dst[..len].copy_from_slice(&src[..len]);
}

fn fixed_str(bytes: &[u8]) -> &str {
    let len = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..len]).unwrap_or("")
}

fn collect_epubs<D, T, const MAX_DIRS: usize, const MAX_FILES: usize, const MAX_VOLUMES: usize>(
    dir: &embedded_sdmmc::Directory<'_, D, T, MAX_DIRS, MAX_FILES, MAX_VOLUMES>,
    prefix: &str,
    in_books_dir: bool,
    library: &mut ReaderStore,
) where
    D: embedded_sdmmc::BlockDevice,
    T: TimeSource,
{
    let mut lfn_storage = [0u8; 192];
    let mut lfn_buffer = LfnBuffer::new(&mut lfn_storage);
    let _ = dir.iterate_dir_lfn(&mut lfn_buffer, |entry, long_name| {
        if entry.attributes.is_directory() || entry.attributes.is_volume() {
            return;
        }

        let mut name = String::<64>::new();
        let mut open_name = String::<16>::new();
        use core::fmt::Write;
        let _ = write!(open_name, "{}", entry.name);
        let Some(file_name) = long_name else {
            let _ = write!(name, "{}", entry.name);
            if !is_epub_name(&name) {
                return;
            }
            push_prefixed(prefix, &name, &open_name, in_books_dir, entry.size, library);
            return;
        };

        if is_epub_name(file_name) {
            push_prefixed(
                prefix,
                file_name,
                &open_name,
                in_books_dir,
                entry.size,
                library,
            );
        }
    });
}

fn push_prefixed(
    prefix: &str,
    name: &str,
    open_name: &str,
    in_books_dir: bool,
    byte_size: u32,
    library: &mut ReaderStore,
) {
    let mut path = String::<64>::new();
    let _ = path.push_str(prefix);
    let _ = path.push_str(name);
    library.push(&path, open_name, in_books_dir, byte_size);
}

fn is_epub_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 5 {
        return false;
    }
    let ext = &bytes[bytes.len() - 5..];
    ext[0] == b'.'
        && ext[1].eq_ignore_ascii_case(&b'e')
        && ext[2].eq_ignore_ascii_case(&b'p')
        && ext[3].eq_ignore_ascii_case(&b'u')
        && ext[4].eq_ignore_ascii_case(&b'b')
}
