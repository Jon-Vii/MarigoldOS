# Bench workflow

Use `tools/bench/bench.py` for hardware-facing development benches. The
emulator and golden frames remain the fast behavior oracle; bench runs answer
board-specific timing, SD/cache, sleep, and soak questions.

## When to run

- Run `tools/bench/bench.py channel-stress --host` during normal development
  when changing reader state, display command, storage command, sync session,
  refresh plan, or queue/coalescing behavior. This needs no hardware.
- Run short hardware confidence checks before trusting a flashed firmware after
  display flush, input debounce, sleep/power, reader rendering, SD session,
  section cache, or progress-write changes:

```sh
tools/bench/bench.py page-turn --port /dev/cu.usbmodem101 --turns 50
tools/bench/bench.py sleep-sync --port /dev/cu.usbmodem101 --cycles 5
tools/bench/bench.py storage-cache --port /dev/cu.usbmodem101 --reset-before --seconds 20 --strict
```

- Run longer hardware checks before releases or risky merges:

```sh
tools/bench/bench.py reader-soak --port /dev/cu.usbmodem101 --minutes 30
tools/bench/bench.py storage-cache --port /dev/cu.usbmodem101 --cold --warm
tools/bench/bench.py sleep-sync --port /dev/cu.usbmodem101 --cycles 20
```

- Run `thermal-run` only for targeted refresh, ghosting, sleep-screen,
  enclosure, power, SD-card, or ambient-temperature investigations.
- `reader-soak` is a passive capture: the operator runs the described
  reading workflow on the device by hand while bench.py records. Menus
  idle-sleep after 3 minutes (Reading after 10), so keep interacting.
- Deep sleep drops the USB-JTAG serial port mid-capture; bench.py
  announces the loss and waits for the port to re-enumerate — wake the
  device to resume. The capture window keeps counting while it is away.

## Logs

Raw bench logs are written under `target/bench/` by default and should not be
committed. Captures append to the same file, so a log usually holds several
runs; `report` (and the summary each capture prints) covers only the latest
run — pass `--all` to pool the whole log.

```sh
tools/bench/bench.py report target/bench/latest.jsonl
```

Keep notable hardware findings in `.scratch/` issues or dated docs notes.
