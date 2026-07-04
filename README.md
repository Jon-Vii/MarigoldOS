# xteink-x4-os

A light OS for the Xteink X4 e-reader.

![Home screen](docs/home.png)

The Xteink X4 is my daily driver for reading ebooks. It is pocket-sized,
with a 4.3″ e-ink display, a 650 mAh battery, a magnetic back, and an
SD-card slot prefitted with a 16 GB card. This firmware reads
full-length EPUBs from the card; books are added and removed through
your browser. There is no heap allocation in the reading path.

*Frames in this README are rendered by the host emulator, pixel-identical
to what the firmware writes to the panel.*

## What it does

- **Landscape** — every surface renders landscape; the X4 is held
  sideways for its page buttons.
- **EPUB reading** — streaming ZIP → XHTML parse into a bounded
  whole-book pagination cache on the card; a cached book reopens in tens
  of milliseconds.
- **Typography** — Literata in four styles, pre-rendered to bitmap
  glyphs on the host; adjustable size and line spacing, and a spacing
  change repaginates without reparsing the book.
- **Library** — the shelf streams from a catalog snapshot on the card;
  library size is not bounded by RAM.
- **Wireless** — the device joins your Wi-Fi and serves a shelf page on
  your LAN: list, upload, and delete books from any browser. The radio
  needs ~100 KB of heap the firmware does not have, so a session loans it
  out of the reader's own scratch buffers and ends with a reset that
  hands them back.
- **Onboarding** — with no stored credentials, the device raises an open
  `XTEINK-X4` hotspot with a captive portal and an on-screen QR code.
- **Power** — idle ends in a sleep screen, then the panel and the SoC
  enter deep sleep; the power button takes the same path.

## The numbers

| | |
|---|---|
| Page turn | 473 ms end-to-end; 421 ms of that is the panel's rated fast waveform |
| Wake from sleep | one flicker, ~1.5 s |
| Cold-boot full refresh | 3.5 s |
| Reopen a cached book | tens of milliseconds |
| RAM | 400 KB SRAM, no PSRAM |
| Usable stack | ~43 KB |
| Framebuffer | one, 48 KB, 1 bit per pixel |

Internals: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

## Build, flash, test

```sh
cargo run -p fw --release                                       # build, flash, serial monitor
cargo test -p app-core -p proto --target aarch64-apple-darwin   # host tests
cargo run --manifest-path tools/emulator/Cargo.toml --target aarch64-apple-darwin \
  --no-default-features -- --scenario fixtures/scenarios --check fixtures/golden
```

Only flashing needs the device on USB; the app logic, parsers, renderer,
and emulator all build and test on a plain host. The nightly toolchain is
pinned in `rust-toolchain.toml`.

## Credits

- [Literata](https://github.com/googlefonts/literata) (OFL) for the reading typeface
- [esp-hal](https://github.com/esp-rs/esp-hal) and [Embassy](https://embassy.dev) underneath everything
- The OpenX4 community SDK for the panel's addressing behavior

## License

MIT
