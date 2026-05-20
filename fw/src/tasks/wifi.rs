use embassy_time::Timer;
use esp_hal::peripherals::WIFI;

#[embassy_executor::task]
pub async fn run(_wifi: WIFI) {
    loop {
        Timer::after_secs(60).await;
    }
}
