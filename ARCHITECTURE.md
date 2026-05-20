# xteink-x4-os architecture

This firmware is a bare-metal Rust bring-up for the Xteink X4 e-ink reader:
ESP32-C3, SSD1677, 800x480 monochrome panel, no PSRAM.

The design goal is not to imitate a desktop OS. It is a small data pipeline:

```text
buttons -> app state -> display command -> framebuffer -> SSD1677 RAM -> refresh -> sleep
```

## Rules

- `#![no_std]`, no heap allocation in firmware paths.
- One 48 KB 1 bpp framebuffer.
- Display ownership is single-writer: only `display_task` touches the EPD bus.
- Reader state ownership is single-writer: only `app_task` mutates page/menu state.
- Messages are small `Copy` values. Bulk bytes stay in caller-owned buffers.
- Power requests display sleep through `display_task`; it never touches SPI.
- Hardware assumptions live in one of two places:
  - X4 board wiring in `fw/src/main.rs` and `fw/src/tasks/input.rs`.
  - SSD1677 protocol in `display/src/epd.rs`.

## Workspace

```text
display/   framebuffer, drawing primitives, SSD1677 constants and address math
hal-ext/   thin async wrappers over ESP HAL peripherals
fw/        boot, Embassy executor, task wiring, board-owned peripherals
ui/        bounded layout data structures for later UI work
proto/     bounded book/storage/text models plus ZIP/EPUB/XHTML parser pieces
```

## Embassy tasks

```text
app_task
  owns ReaderState
  InputEvent -> DisplayCommand::Render
  modes: Library, Reading, Chapters, Settings

display_task
  owns EpdBus and Framebuffer
  DisplayCommand::Render -> framebuffer render -> BW/RED RAM write -> full refresh
  DisplayCommand::Sleep -> SSD1677 deep sleep -> PowerEvent::DisplayAsleep
  sends DisplayEvent::Settled to app_task when render completes

input_task
  polls GPIO3 and ADC ladders
  debounced ADC/power edges -> reader Button actions -> InputEvent

power_task
  observes activity and display-settled events
  asks display_task to sleep the SSD1677, then enters ESP32-C3 deep sleep

wifi_task
  parked placeholder until sync becomes a real phase
```

Embassy is used for cooperative waits: ADC retry delays, button polling, SPI DMA
transfers, BUSY waits, and sleep windows all yield instead of spinning. The real
battery win comes after display settle: the power task asks the display task to
power down the SSD1677, then moves the ESP32-C3 into deep sleep.

Input/render backpressure is intentionally coalesced. The app keeps at most one
display render in flight. While the display is refreshing, new button events
still update `ReaderState`, but they set a single pending-render flag instead of
queuing stale framebuffer renders. When `DisplayEvent::Settled` arrives, the app
renders the latest state once.

## Display model

`display::fb::Framebuffer` is the source of truth. White is bit `1`, black is
bit `0`, row-major, 100 bytes per row.

The SSD1677 path writes the current framebuffer to BW RAM (`0x24`). The first
refresh after boot/display sleep also writes the current framebuffer to RED RAM
(`0x26`) and runs a full waveform. Normal page turns use a second retained
framebuffer as the RED RAM previous-frame source, then trigger the SSD1677 fast
waveform. This avoids the multi-flash full-update behavior during ordinary
reader navigation. Periodic full-refresh cleanup is available behind a constant
but currently disabled so page-turn behavior is deterministic during bring-up.

`display_task` contains three transform constants currently validated during bring-up:
`MIRROR_X = true`, `MIRROR_Y = false`, and `REVERSE_BITS = true`. The logical
framebuffer API stays upright while the task remaps bytes/bits before DMA
streaming. This fixes the X4 panel's observed horizontal byte order and bit
order without leaking hardware orientation into app rendering. `MIRROR_Y=true`
was tested and rejected because it made glyphs vertically mirrored/upside down.

Physical orientation is an app/layout concern, not an SSD1677 streaming concern.
The current readable build places logical top on the physical button side. The
reader state already carries a complete orientation enum:

```rust
enum DisplayOrientation {
    LandscapeButtonsBottom,
    LandscapeButtonsTop,
    PortraitButtonsLeft,
    PortraitButtonsRight,
}
```

Default reader mode is `LandscapeButtonsBottom`, but the low-level display
transform above should stay fixed unless corruption returns.

Addressing follows the OpenX4 community SDK behavior:

- SPI mode 0, 40 MHz.
- BUSY is active high.
- X window is pixel-addressed, `0..799`.
- Y gate scan is reversed, so the full Y window is `479..0`.

## Data-oriented design

State is plain data, not object graphs:

```text
InputEvent        Copy enum
ReaderState       view/book/page/chapter/settings/battery fields
RenderRequest     view/book/page/orientation/refresh/battery/dirty rect
Layout<N>         parallel arrays of kind/rect/parent/text span
Framebuffer       single flat byte array
```

EPUB work keeps the same shape:

```text
flash bytes -> zip entry -> inflate window -> XML token -> line box -> glyph blit
```

No DOM, no `Vec<Node>`, no retained chapter tree unless measurements prove it is
worth the memory.

`proto` owns the reader data contracts shared by Home, Files, Reading, and
Chapters:

- `BookMeta`, `BookProgress`, and `ChapterMeta` for catalog and progress data.
- `BookStorage` and `FileCandidate` for microSD-backed `.epub` discovery.
- `ZipArchive` for central-directory lookup and stored/deflated entry reads into
  caller-owned buffers.
- `ZipStream` for central-directory lookup and entry reads through a bounded
  `ReadAt` interface, which is the path storage-backed EPUBs use.
- `EpubPackage` for container/OPF metadata, manifest, and spine.
- `TextRun`, `TextRole`, `FontStyle`, and `paginate_screen` for text-only XHTML
  reading and deterministic one-screen pagination.

The firmware still ships one built-in catalog entry while the physical microSD
FAT scanner is finished. The X4 SD pins are configured on the shared SPI bus
(SCK GPIO8, MOSI GPIO10, MISO GPIO7, SD CS GPIO12), but real SD transactions are
kept behind the pending single board-I/O owner so display refresh and SD reads
cannot overlap.

Reading typography uses generated Literata bitmap assets. The host generator
downloads OFL Literata TTFs and emits Latin-1 glyph metrics/bitmaps for Regular,
Italic, Bold, and BoldItalic. Firmware does not rasterize TTFs on-device.

## Reader app model

The firmware now has the e-reader surfaces as explicit app state:

- `Home`: current book cover/metadata plus Continue, Library, and Settings.
- `Library`: selects a book or opens settings.
- `Reading`: owns the active book/page position.
- `Chapters`: selects a chapter within the current book.
- `Settings`: cycles orientation and refresh policy.

The interface is split by context. Device/navigation surfaces (`Home`,
`Library`, `Settings`) render in portrait because covers, lists, and settings are
naturally vertical. Book surfaces (`Reading`, `Chapters`) stay in landscape for
the current reading posture. Home is cover-led: the current book is the visual
anchor, with a restrained bottom tab strip for Read, Library, and Settings.
Reading mode keeps the page quiet: tiny book title, rendered-screen count within
the chapter, symbolic battery, and a thin whole-book progress bar. Home shows a
small battery percentage because it is a status surface. GPIO0 is sampled as the
current rough battery source using a 2:1 divider assumption and a simple
3300-4200 mV LiPo percentage curve. The current book is a built-in catalog entry
backed by static text pages. Real EPUB support now has parser/storage contracts
in `proto`; the next hardware step is FAT scanning on microSD, filling
`BookMeta` from `/books/*.epub`, then opening the selected EPUB into the same
current book id, chapter index, and screen offset fields.

Persistent app state is represented by `hal_ext::nvm::AppStateRecord`, a compact
versioned/checksummed record for book id, chapter, rendered screen, shell
orientation, reading orientation, and refresh policy. Actual flash writes are
still pending; the record format is intentionally separate from ESP flash driver
choice.

## Bring-up checklist

1. Flash firmware and confirm the reader shell appears.
2. Measure BUSY on GPIO6 during reset and refresh.
3. Confirm full refresh timing.
4. Confirm `TL`, `TR`, `BL`, and `BR` are readable and map consistently.
   Current readable transform: `MIRROR_X=true`, `MIRROR_Y=false`,
   `REVERSE_BITS=true`. Logical top currently appears on the physical button
   side; handle this later through `DisplayOrientation`.
5. Validate the Adafruit-scaled ADC ladder bands against this physical unit.
   Current calibrated bands are GPIO1 Back `2400..2700`, Confirm `1800..2150`,
   Left `1000..1250`, Right `0..100`; GPIO2 Up `1500..1800`, Down `0..100`. Raw
   hardware buttons then pass through a CrossPoint-style mapping layer into
   reader actions: front `BACK_CONFIRM_LEFT_RIGHT`, side `PREV_NEXT`. Both
   previous-page buttons emit `Previous`; both next-page buttons emit `Next`.
   Raw ADC serial logging and on-screen GPIO values are now behind debug
   constants so normal firmware only refreshes on debounced button edges.
6. Measure deep-sleep current.
7. Only then add partial refresh, NVM progress, storage, and Wi-Fi sync.
