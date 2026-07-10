# CalendulaOS Rebrand — PRD

Status: **planning, decisions closed.** Name, palette, and lineage handling all
agreed 2026-07-09. No code written yet. This document is the shared understanding
to build against.

## Summary

Rebrand this fork from **MarigoldOS** to **CalendulaOS**. The upstream repo
(Jon-Vii's, formerly `xteink-x4-os`) now also carries the MarigoldOS name, so the
fork and its parent are indistinguishable — same wordmark, same flower, same
release URLs. Worse than ambiguous: the fork's site and flasher currently point at
*upstream's* releases and Pages URLs, so it doesn't just share the name, it wears
upstream's entire identity.

This is a rename-and-repoint, not a rewrite. Two threads:

1. **Brand** — new name, a hue-shifted palette, and a redrawn flower mark, swept
   through the device string, the site, tooling, and docs.
2. **Identity infrastructure** — the fork cuts its own releases and serves its own
   site, so every `Jon-Vii/marigold-os` URL becomes `chongfun/calendula-os`.

## Brand: CalendulaOS

*Calendula* is the pot marigold — the sibling marigold genus. Same family,
unmistakably different word: the name keeps the lineage legible while ending the
ambiguity.

- **Positioning:** unchanged — open-source firmware for the Xteink X3 / X4
  e-reader. The pitch line stays; only the name in it changes.
- **Hue shift — "deep amber," decided:** upstream's accent is vivid orange
  (`--mari: #cf5f0c`, petals `#f5871c`). The fork's palette is seeded from the
  **LED amber already in the page** (`--led: #c98a2b`) — the same move the pages
  PRD made with marigold, replayed in the fork's own key. Concrete values,
  WCAG-checked against the page's real backgrounds:

  | Role | Light mode (paper `#eeece2`) | Dark mode (card `#241f17`) |
  |---|---|---|
  | Accent `--cal` | `#a16207` (4.16:1 — beats upstream's 3.35) | `#f0b429` (8.78:1) |
  | Button fill | `#e8b40c → #c98a2b` | same |
  | Button edge | `#8a6200` | same |
  | Button text | **ink `#161614`** (9.46:1) | same |
  | Desk `--desk` (flat) | `#ffffff` (GitHub light canvas) | `#0d1117` (GitHub dark canvas) |

  The desk is **GitHub's Primer canvas, flat** (gradients and the amber glow
  were dropped — they'd seam at the image edges): README hero screenshots blend
  into github.com. The README uses a `<picture>` element with light and dark
  captures (`docs/home.png` / `docs/home-dark.png`) so it's seamless in both
  themes; a calendula-gold desk was tried first and rejected as too loud. The
  shell reads as an object via its edge ring and shadows (ink 18.1:1, labels
  7.7:1 / 5.0:1 on the two canvases).

  Golden fills are too light for white text — the flash button's label goes
  **ink-dark**, a deliberate difference from upstream's button. In-situ nudging
  during the site work is fine; these values are the spec, not a mood board.
  At ~35° hue vs upstream's ~24°, a screenshot or browser tab is tellable apart
  at a glance while staying in the warm marigold-family register.
- **New mark:** `web/marigold.svg` (a 20-petal two-ring marigold) is replaced by a
  redrawn **calendula** — many narrow petals, single dense ring, in the new golden
  accent — as `web/calendula.svg`. It serves both roles the old file did: favicon
  and header wordmark icon.
- **The discipline holds:** the hue shift applies only where the brand has color —
  site, wordmark, buttons. The emulator's Mist Gray device shell and the
  ink-on-paper panel stay exactly as they are. Warm in every context that has
  color, pure ink the moment it's on the panel — the same tension the pages PRD
  articulated, in a new key.

## What stays (not brand)

These are contracts and physical facts, not branding. None of them change:

- **SD-updater filenames** — `update.bin`, `FWUPDX3.BIN` (OEM updater scan names)
  and `FWUPDATE.BIN` (the in-app updater trigger). The device scans for these exact
  names; they are contracts, not cosmetic.
- **Release asset names** — `firmware-x4.bin` / `firmware-x3.bin`, flashed
  app-image-only at **`0x10000`**. The whole flasher posture (no `full-flash`,
  ever) carries over untouched.
- **Device shell + panel rendering** — the CSS shell, the `present()` mapping, the
  golden-frame pipeline.
- **Partition layout, build system, toolchain** — the rebrand renames strings and
  URLs; it does not touch how the firmware is built or laid out.

## Rename surface

Everywhere the old name lives, grouped by what kind of change it is.

**On-device (one string, two stamps):**

- `fw/src/views.rs:80` — the `"MarigoldOS"` shown on the device (sleep-blank
  screen). This is the only brand string compiled into the firmware.
- **App-descriptor stamps** (`fw/src/main.rs`) — the ESP app descriptor's
  `project_name` and `version` fields are both fixed `[u8; 32]` arrays, currently
  all zeros. Fill them: `project_name = "CalendulaOS (MarigoldOS)"` and `version`
  from the crate version at compile time (`env!("CARGO_PKG_VERSION")` through a
  const fn). Both fields are already in every image at full size, so this costs
  **zero extra bytes** — it's the free lineage + version stamp, readable off any
  device or `.bin` via `esptool image_info`.

**Site (`web/`):**

- `index.html` — `<title>`, og/twitter meta (title, image alt), header wordmark,
  hero `<h1>`, the emulator blurb, the "Flash MarigoldOS" section heading, the
  in-app-updater copy ("Already running Marigold on an X4…"), and the docs links.
- `marigold.svg` → `calendula.svg` (redrawn, see Brand above); favicon `<link>` and
  the header `<img>` follow.
- `manifest-x4.json` / `manifest-x3.json` — `"name": "MarigoldOS X4/X3"` →
  `"CalendulaOS X4/X3"` (this is the name esp-web-tools shows in the install
  dialog).
- The site's accent variables (`--mari`, `--mari-btn1/2`, `--mari-edge`, both
  light and dark blocks) retune to the calendula golden accent; rename the
  variables to `--cal*` while touching them so the CSS doesn't lie.

**URLs — the repoint (site + manifests):**

Every external URL in `web/` currently points at upstream. All of them move to the
fork:

| Where | From | To |
|---|---|---|
| `manifest-x*.json` firmware paths | `github.com/Jon-Vii/marigold-os/releases/latest/download/…` | `github.com/chongfun/calendula-os/releases/latest/download/…` |
| `release-assets.json` (SD downloads + notes link) | same | same |
| `index.html` `const R=` download base | same | same |
| `index.html` GitHub / docs / release-notes links | `github.com/Jon-Vii/marigold-os/…` | `github.com/chongfun/calendula-os/…` |
| og/twitter URL + image | `jon-vii.github.io/marigold-os/` | `chongfun.github.io/calendula-os/` |

The hardcoded "v0.2.1 release notes" line in `index.html` gets re-pointed with the
rest and updated to whatever the fork's first release is (see below).

**Tooling / CI:**

- `tools/cargo.sh` — the "rustup is required to build MarigoldOS firmware" error.
- `tools/build-release.sh` — the Marigold references in comments and the final
  summary echo.
- `.github/workflows/release.yml` — the release-notes string ("MarigoldOS
  ${GITHUB_REF_NAME} firmware images…").

**Docs (mechanical sweep, ~15 hits):**

`README.md`, `docs/FLASHING.md`, `docs/CONTEXT.md`, `docs/ARCHITECTURE.md`,
`docs/CUSTOM_FONTS.md`, `tools/bench/README.md`, `tools/bench/bench.py`,
`tools/build_font_pack.py`. Existing `docs/plans/` and `docs/brainstorms/` entries
are historical records — they keep their original names and are *not* rewritten.

**Screenshots:**

`docs/home.png` (also the og:image) shows the MarigoldOS wordmark in the page
chrome. Regenerate after the site rename lands, so the social preview doesn't
advertise the old brand.

**The repo itself:**

Rename `chongfun/marigold-os` → `chongfun/calendula-os` on GitHub. GitHub
redirects the old repo URL and git remotes, so nothing breaks in the transition —
but local remotes and the local directory name should be updated as a cleanup
rider, not left to the redirect forever (the lesson from the last rename, which
left the local checkout reading `xteink-x4-os` for weeks).

## Own release identity

The fork's flasher can't point at upstream's releases once the two diverge — and
pointing at them today is already wrong (upstream's binaries aren't built from
this tree). The rebrand therefore includes cutting the fork's **first own
release**:

- The tag-triggered `release.yml` already exists and needs only its notes string
  updated — tag a `v*` on the fork and it produces all four assets.
- **Versioning:** continue upstream's semver line rather than restarting —
  the firmware lineage is shared and a reset to `v0.1.0` would read as a
  regression. First CalendulaOS release: **`v0.4.0`** (upstream is at `v0.3.2`;
  the minor bump marks the identity change).
- **Tag ↔ crate-version sync:** the descriptor's `version` stamp comes from the
  `fw` crate version, so bumping `fw`'s `Cargo.toml` version becomes part of
  cutting a release — tag `v0.4.0` means crate version `0.4.0`, or the stamp lies.
- Sequence matters, harder than first thought: upstream's `pages.yml` now
  **downloads the latest release's firmware into the site at build time** (the
  manifests use relative paths), so with no release on the fork the Pages deploy
  itself fails — observed on the first post-rebrand push ("release not found").
  Cutting `v0.4.0` is what unblocks the fork's site deploy, not just the flasher.

## Upstream relationship

- **Credit, visibly:** README and the site footer both state the lineage — a fork
  of Jon-Vii's MarigoldOS — with a link. Good citizenship, and it helps users who
  arrive from upstream understand what they're looking at.
- **On-device lineage — resolved by a zero-byte rule:** a *visible* mention (a
  second line on the sleep-blank screen) would cost real bytes — a new `.rodata`
  string plus another draw call — so it's **out**. The app-descriptor
  `project_name` stamp (see Rename surface) carries the lineage instead, at
  exactly zero bytes: `"CalendulaOS (MarigoldOS)"` baked into every image, just
  not rendered. Repo link and README credits do the human-facing work.
- **Merge cost, consciously accepted:** the rename touches ~16 files upstream also
  touches (`index.html` especially). Every future upstream merge will conflict on
  brand strings. That's the price of disambiguation; the sweep keeps changes as
  string-level as possible (no restructuring while renaming) to keep those
  conflicts trivial.

## Build sequence

1. **Repo rename** on GitHub (`chongfun/calendula-os`) — redirects make this safe
   to do first, and every subsequent URL edit can be written against the final
   name.
2. **Name sweep** — device string, app-descriptor stamps (`project_name` +
   `version`), tooling, CI, docs, plus all non-release URLs (repo, docs,
   og/Pages links) — those are safe to move the moment the repo is renamed.
   Mechanical; one commit.
3. **New mark + palette** — draw `calendula.svg`, retune the accent variables,
   rename `--mari*` → `--cal*`, update wordmark/meta in `index.html`.
4. **Cut `v0.4.0`** — first CalendulaOS release, so `releases/latest/download/`
   resolves under the fork.
5. **Repoint the release-coupled URLs** — `release-assets.json`, the
   `FWUPDATE.BIN` and release-notes links, and the displayed version label;
   these must wait for the `v0.4.0` release or they 404.
6. **Regenerate `home.png`** — screenshot with the new brand for README + og:image.

## Cleanup riders (not blockers)

- **Local checkout** — rename the local directory and `origin` remote to
  `calendula-os` so the working copy matches the remote (don't repeat the
  `xteink-x4-os` lag).
- **`--mari*` CSS variables** — renamed in step 3 above; listed here in case the
  palette work ships without it.

## Open questions / deferred

None — all three from the first draft were resolved 2026-07-09 and folded into the
body above. The rule that resolved the firmware-side pair, kept for future calls of
the same shape: **on-device niceties happen only when they cost zero extra binary
bytes.** Lineage and version both fit inside the app descriptor's fixed, zeroed
`[u8; 32]` fields (free → done); rendering either on screen costs string + code
bytes (→ not done; README, site footer, and `esptool image_info` cover it).
