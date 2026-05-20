#![no_std]
#![no_main]
#![allow(clippy::empty_loop)]
#![allow(clippy::manual_div_ceil)] // False positive inside esp_hal::dma_buffers!.

use esp_hal::dma::{Dma, DmaPriority};
use esp_hal::entry;
use esp_hal::gpio::Io;
use esp_hal::prelude::*;
use esp_hal::spi::master::Spi;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[entry]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma
        .channel0
        .configure_for_async(false, DmaPriority::Priority0);

    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(8000);
    let dma_rx_buf = esp_hal::dma::DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = esp_hal::dma::DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let _spi = Spi::new(peripherals.SPI2, 10_u32.MHz(), esp_hal::spi::SpiMode::Mode0)
        .with_sck(io.pins.gpio8)
        .with_mosi(io.pins.gpio10)
        .with_dma(dma_channel)
        .with_buffers(dma_rx_buf, dma_tx_buf);

    loop {}
}
