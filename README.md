# CalendulaOS

CalendulaOS is a lightweight, open-source firmware for the Xteink X4 and X3
e-readers — a fork of [Jon-Vii's MarigoldOS](https://github.com/Jon-Vii/marigold-os).

[![Flashing](https://img.shields.io/badge/Flashing-2b2b2b?style=flat)](docs/FLASHING.md)
[![Custom fonts](https://img.shields.io/badge/Custom_fonts-2b2b2b?style=flat)](docs/CUSTOM_FONTS.md)
[![Architecture](https://img.shields.io/badge/Architecture-2b2b2b?style=flat)](docs/ARCHITECTURE.md)

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/home-dark.png">
  <img alt="The CalendulaOS site showing the browser emulator home menu with Alice's Adventures in Wonderland selected" src="docs/home.png">
</picture>

If you'd like to explore the OS without flashing a device,
[try the emulator](https://chongfun.github.io/calendula-os/) in your browser — the
firmware's app and rendering code compiled to WebAssembly, with a simulated
e-ink display and a selection of public-domain books.

## Features

### Reading
- **EPUB 2 & 3** — native table of contents for each (EPUB 3 nav, NCX fallback)
- **Reader typefaces** — Literata and Merriweather, plus an optional custom typeface slot from the SD card
- **Whole-book pagination cache** — a book parses once and reopens in tens of milliseconds
- **Fast page turns** — 473 ms end-to-end, within ~50 ms of the panel's rated floor

### Library & sync
- **Streamed catalog** — library size isn't bounded by RAM
- **Local Wi-Fi shelf** — upload, list, and delete books from your browser
- **Zero-config onboarding** — with no stored credentials, the reader raises a WPA2-protected, board-specific hotspot with a freshly generated per-session password presented on-screen, a captive portal, and an on-screen QR code

### Try it
- **Browser emulator** — the real render code in WebAssembly, no device needed

## Development

Prerequisites: install Rust with `rustup`, then install the firmware target and
release-image tool:

```sh
rustup target add riscv32imc-unknown-none-elf
cargo install espflash
./tools/install-hooks.sh                                        # optional: git hooks for local feedback
```

```sh
tools/cargo.sh run -p fw --release                              # build, flash, serial monitor
tools/check.sh test-host                                        # host tests
tools/check.sh golden-frames                                    # X4/X3 emulator goldens
tools/bench/bench.py channel-stress --host                       # host bench
```

Only flashing needs the device on USB; the app logic, parsers, renderer, and
emulator all build and test on a plain host. The stable toolchain is configured in
`rust-toolchain.toml`.

Hardware-facing bench runs live in `tools/bench`. Use short `page-turn` and
`sleep-sync` runs before trusting flashed builds after display, input, sleep,
reader rendering, SD/cache, or progress-write changes; use longer soak/storage
runs before risky merges or releases.

The board is a compile-time feature (X4 by default, or `device-x3`); pass
`--features device-x3` to run the firmware, native emulator, or preview for
the X3. The browser emulator builds one wasm per board:

```sh
tools/build-web.sh _site                # X4 + X3 wasm + index.html into _site/
python3 -m http.server -d _site 8000    # then open http://localhost:8000/?board=x3
```

`.github/workflows/pages.yml` runs this on push to `main`; the hosted page's
`?board=` switch picks which wasm to load (default X4).

## Flashing

`cargo run` flashes over USB for development. To install without a toolchain —
from a built image, or onto a unit that shipped with USB flashing disabled —
tagged releases publish the app/SD images and [docs/FLASHING.md](docs/FLASHING.md)
covers the paths: web flasher, SD card, and the in-app update from the card.

## Credits

- [Literata](https://github.com/googlefonts/literata) and [Merriweather](https://github.com/SorkinType/Merriweather) (both OFL) for the reading typefaces
- [The OpenX4 community SDK](https://github.com/open-x4-epaper/community-sdk) for panel addressing behavior
- [Crosspoint Reader](https://github.com/crosspoint-reader/crosspoint-reader) for the community reverse-engineering behind X3 device support

## License

MIT
