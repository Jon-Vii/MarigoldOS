//! UC8253 flush backend — Xteink X3. Ported from CrossPoint's production
//! `Uc8253X3Driver` (MIT), BW path only (no grayscale reader here).
//!
//! Plane model: DTM1 holds the "old" frame, DTM2 the "new". A `Fast` turn
//! diffs DTM2 against DTM1 with the turbo bank; `Full` writes a white-DTM1
//! baseline then a settle pass; `FastClean` scrubs every pixel to target
//! with the absolute half bank. As on the X4, `prestage_previous` stages
//! the just-shown frame into the old plane (DTM1) off the page-turn
//! critical path, so the next `Fast` finds its diff base already loaded —
//! `flush`'s `prev_staged` says whether that happened.
//!
//! UNVERIFIED on hardware (plan Phase 6): the two-phase BUSY timing, panel
//! orientation, and whether the ported waveforms hold at the chosen SPI
//! clock are all first-boot iteration points.

use super::{Epd, SpiError};
use display::epd::uc8253::{
    bank_for, LutBank, CDI_INTERVAL, CMD_DATA_STOP, CMD_DEEP_SLEEP, CMD_DISPLAY_REFRESH, CMD_DTM1,
    CMD_DTM2, CMD_LUT_BB, CMD_LUT_BW, CMD_LUT_VCOM, CMD_LUT_WB, CMD_LUT_WW, CMD_POWER_OFF,
    CMD_POWER_ON, CMD_VCOM_DATA_INTERVAL, DEEP_SLEEP_CHECK, INIT_SEQUENCE,
};
use display::epd::uc8253::fill_transformed_band;
use display::epd::RefreshMode;
use display::fb::Framebuffer;
use display::{BAND_BYTES, BAND_ROWS, HEIGHT, ROW_BYTES};
use embassy_time::Timer;
// riscv32imc has no CAS; portable-atomic provides plain load/store here.
use portable_atomic::{AtomicBool, Ordering};

/// Mirrors the controller's charge-pump state (`_isScreenOn`). The panel is
/// left powered between page turns for speed; only `sleep_panel` powers it
/// down. `init_panel` resets it to off. Single writer (the display task).
static SCREEN_POWERED: AtomicBool = AtomicBool::new(false);

/// Settle delay after a non-fast refresh, matching the reference's 200 ms.
const SETTLE_MS: u64 = 200;

pub(crate) async fn init_panel(epd: &mut Epd) {
    // Bring-up probe: a live UC8253 twitches BUSY across a hardware reset;
    // a line that reads high at every sample never left power-up, which is
    // a board/RST problem, not a command-stream one.
    esp_println::println!("display: x3 busy pre-reset high={:?}", epd.busy_is_high());
    epd.reset().await;
    esp_println::println!("display: x3 busy post-reset high={:?}", epd.busy_is_high());
    // The X3 needs an extra settle after reset beyond the shared pulse.
    Timer::after_millis(50).await;

    for (cmd, data) in INIT_SEQUENCE {
        let _ = epd.command(*cmd, data).await;
    }
    esp_println::println!("display: x3 busy post-init high={:?}", epd.busy_is_high());

    // UC8253 has no auto RAM clear; whiten both planes so the first
    // differential diffs against white rather than power-on garbage.
    let _ = fill_plane(epd, CMD_DTM1, 0xFF).await;
    let _ = fill_plane(epd, CMD_DTM2, 0xFF).await;

    SCREEN_POWERED.store(false, Ordering::Relaxed);
    esp_println::println!("display: x3 init done");
}

pub(crate) async fn flush(
    epd: &mut Epd,
    fb: &Framebuffer,
    prev_fb: &Framebuffer,
    tx_band: &mut [u8; BAND_BYTES],
    _screen_on: bool,
    mode: RefreshMode,
    prev_staged: bool,
) -> Result<(), SpiError> {
    // With the pump off (fresh init or post-sleep) DTM1 no longer matches
    // what the panel shows, so a differential fast turn would mis-drive.
    // The reference upgrades that case to the absolute half scrub, which
    // ignores DTM1 entirely.
    let mode = if mode == RefreshMode::Fast && !SCREEN_POWERED.load(Ordering::Relaxed) {
        RefreshMode::FastClean
    } else {
        mode
    };
    esp_println::println!("display: x3 flush {:?}", mode);
    match mode {
        RefreshMode::Full | RefreshMode::PowerDown => flush_full(epd, fb, tx_band).await,
        RefreshMode::Fast => flush_fast(epd, fb, prev_fb, tx_band, prev_staged).await,
        RefreshMode::FastClean => flush_clean(epd, fb, tx_band).await,
    }
}

/// Full quality write: white DTM1 baseline + `full` bank, then a no-op fast
/// settle (the first differential after a full garbles on the X3, so we
/// spend one silent fast refresh of the same frame). Leaves DTM1 == fb.
async fn flush_full(
    epd: &mut Epd,
    fb: &Framebuffer,
    tx_band: &mut [u8; BAND_BYTES],
) -> Result<(), SpiError> {
    let (bank, cdi) = bank_for(RefreshMode::Full);
    load_bank(epd, cdi, bank).await?;
    fill_plane(epd, CMD_DTM1, 0xFF).await?;
    send_plane(epd, CMD_DTM2, fb, tx_band).await?;
    // Full always re-powers the charge pump (higher current) even if on.
    refresh(epd, true).await?;
    Timer::after_millis(SETTLE_MS).await;

    // Sync DTM1 to the shown frame, then settle it with a silent fast pass.
    send_plane(epd, CMD_DTM1, fb, tx_band).await?;
    epd.command(CMD_DATA_STOP, &[]).await?;
    let (fast_bank, fast_cdi) = bank_for(RefreshMode::Fast);
    load_bank(epd, fast_cdi, fast_bank).await?;
    send_plane(epd, CMD_DTM2, fb, tx_band).await?;
    refresh(epd, false).await?;
    send_plane(epd, CMD_DTM1, fb, tx_band).await?;
    epd.command(CMD_DATA_STOP, &[]).await
}

/// Turbo differential page turn: DTM2 = new frame diffed against DTM1 = the
/// previous frame. When `prev_staged` is false (no prestage since the last
/// refresh) the previous frame is loaded into DTM1 first.
async fn flush_fast(
    epd: &mut Epd,
    fb: &Framebuffer,
    prev_fb: &Framebuffer,
    tx_band: &mut [u8; BAND_BYTES],
    prev_staged: bool,
) -> Result<(), SpiError> {
    let (bank, cdi) = bank_for(RefreshMode::Fast);
    load_bank(epd, cdi, bank).await?;
    if !prev_staged {
        send_plane(epd, CMD_DTM1, prev_fb, tx_band).await?;
        epd.command(CMD_DATA_STOP, &[]).await?;
    }
    send_plane(epd, CMD_DTM2, fb, tx_band).await?;
    refresh(epd, false).await
}

/// One-flicker clean: the absolute `half` scrub bank drives every pixel to
/// its target ignoring DTM1, so no previous-frame plane is needed.
async fn flush_clean(
    epd: &mut Epd,
    fb: &Framebuffer,
    tx_band: &mut [u8; BAND_BYTES],
) -> Result<(), SpiError> {
    let (bank, cdi) = bank_for(RefreshMode::FastClean);
    load_bank(epd, cdi, bank).await?;
    send_plane(epd, CMD_DTM2, fb, tx_band).await?;
    refresh(epd, false).await?;
    Timer::after_millis(SETTLE_MS).await;
    Ok(())
}

/// Stage the just-shown frame into DTM1 ("old" RAM) so the next fast turn's
/// diff base is loaded off the critical path. The X4's `prestage_previous`
/// analogue (RED RAM there, DTM1 here).
pub(crate) async fn prestage_previous(
    epd: &mut Epd,
    fb: &Framebuffer,
    tx_band: &mut [u8; BAND_BYTES],
) -> Result<(), SpiError> {
    send_plane(epd, CMD_DTM1, fb, tx_band).await?;
    epd.command(CMD_DATA_STOP, &[]).await
}

pub(crate) async fn sleep_panel(epd: &mut Epd) -> Result<(), SpiError> {
    if SCREEN_POWERED.load(Ordering::Relaxed) {
        epd.command(CMD_POWER_OFF, &[]).await?;
        let (low, ms) = epd.wait_two_phase().await;
        esp_println::println!("display: x3 POF busy_low={} {}ms", low, ms);
        SCREEN_POWERED.store(false, Ordering::Relaxed);
    }
    epd.command(CMD_DEEP_SLEEP, &[DEEP_SLEEP_CHECK]).await
}

/// Power the charge pump if needed (or unconditionally when `force`, as a
/// full refresh does), fire the refresh, and wait out the two-phase BUSY.
/// The panel is left powered for the next turn.
///
/// Bring-up telemetry: `busy_low=false` on the DRF line means the
/// controller never went busy — the refresh command was ignored, which
/// points at an incomplete RAM write or rejected init rather than the
/// waveform.
async fn refresh(epd: &mut Epd, force: bool) -> Result<(), SpiError> {
    if force || !SCREEN_POWERED.load(Ordering::Relaxed) {
        epd.command(CMD_POWER_ON, &[]).await?;
        let (low, ms) = epd.wait_two_phase().await;
        esp_println::println!(
            "display: x3 PON busy_low={} {}ms level_high={:?}",
            low,
            ms,
            epd.busy_is_high()
        );
        SCREEN_POWERED.store(true, Ordering::Relaxed);
    }
    epd.command(CMD_DISPLAY_REFRESH, &[]).await?;
    let (low, ms) = epd.wait_two_phase().await;
    esp_println::println!(
        "display: x3 DRF busy_low={} {}ms level_high={:?}",
        low,
        ms,
        epd.busy_is_high()
    );
    Ok(())
}

async fn load_bank(epd: &mut Epd, cdi0: u8, bank: &LutBank) -> Result<(), SpiError> {
    epd.command(CMD_VCOM_DATA_INTERVAL, &[cdi0, CDI_INTERVAL])
        .await?;
    epd.command(CMD_LUT_VCOM, bank.vcom).await?;
    epd.command(CMD_LUT_WW, bank.ww).await?;
    epd.command(CMD_LUT_BW, bank.bw).await?;
    epd.command(CMD_LUT_WB, bank.wb).await?;
    epd.command(CMD_LUT_BB, bank.bb).await?;
    Ok(())
}

/// Stream one framebuffer into a RAM plane, band by band, in the panel's
/// row order (the shared band transform applies the X3's vertical flip).
///
/// Deliberately does NOT send DATA_STOP: the hardware-proven reference
/// never puts one between a DTM2 write and the refresh that displays it —
/// only after DTM1 syncs and white fills. Callers add it where the
/// reference does.
async fn send_plane(
    epd: &mut Epd,
    ram_cmd: u8,
    fb: &Framebuffer,
    tx_band: &mut [u8; BAND_BYTES],
) -> Result<(), SpiError> {
    epd.begin_ram_write(ram_cmd).await?;
    let mut y = 0;
    let mut result = Ok(());
    while y < HEIGHT {
        let len = fill_transformed_band(fb, y, tx_band);
        if let Err(err) = epd.ram_chunk(&tx_band[..len]).await {
            result = Err(err);
            break;
        }
        y += BAND_ROWS;
    }
    epd.end_ram_write();
    result
}

/// Fill a RAM plane with a constant byte (0xFF = white), row by row.
async fn fill_plane(epd: &mut Epd, ram_cmd: u8, fill: u8) -> Result<(), SpiError> {
    let mut row = [0u8; ROW_BYTES];
    row.fill(fill);
    epd.begin_ram_write(ram_cmd).await?;
    let mut result = Ok(());
    for _ in 0..HEIGHT {
        if let Err(err) = epd.ram_chunk(&row).await {
            result = Err(err);
            break;
        }
    }
    epd.end_ram_write();
    result?;
    epd.command(CMD_DATA_STOP, &[]).await
}
