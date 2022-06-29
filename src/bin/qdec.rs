#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::info;
use embassy::executor::Spawner;
use embassy::time::Delay;
use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::qdec::{self, Qdec};
use embassy_nrf::{interrupt, spim, Peripherals};
use embedded_graphics::image::{ImageRaw, ImageRawLE};
use embedded_graphics::{image::Image, pixelcolor::Rgb565, prelude::*};
use embedded_hal_async::spi::ExclusiveDevice;
use heapless::String;
use st7735_embassy::ST7735;
use tinybmp::Bmp;
use {defmt_rtt as _, panic_probe as _};
#[embassy::main]
async fn main(_spawner: Spawner, p: Peripherals) {
    // SPI configuration
    let mut config_spi = spim::Config::default();
    config_spi.frequency = spim::Frequency::M32;
    let irq = interrupt::take!(SPIM3);
    // SPIM args: spi instance, irq, sck, mosi/SDA, config
    let spim = spim::Spim::new_txonly(p.SPI3, irq, p.P0_04, p.P0_28, config_spi);
    // CS: chip select pin
    let cs_pin = Output::new(p.P0_31, Level::Low, OutputDrive::Standard);
    let spi_dev = ExclusiveDevice::new(spim, cs_pin);

    // RST:  display reset pin, managed at driver level
    let rst = Output::new(p.P0_30, Level::High, OutputDrive::Standard);

    // DC: data/command selection pin, A0 on this screen, managed at driver level
    let dc = Output::new(p.P0_29, Level::High, OutputDrive::Standard);

    // Config display
    let mut display = ST7735::new(spi_dev, dc, rst, Default::default(), 160, 128);
    display.init(&mut Delay).await.unwrap();
    display.clear(Rgb565::BLACK).unwrap();

    let mut width: i16 = 86;

    let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(
        include_bytes!("../../assets/ferris.raw"),
        width.max(0) as u32,
    );
    let image: Image<_> = Image::new(&image_raw, Point::new(34, 24));
    image.draw(&mut display).unwrap();
    display.flush().await.unwrap();

    // LED is set to max, but can be modulated with pwm to change backlight brightness
    let mut backlight = Output::new(p.P0_03, Level::High, OutputDrive::Standard);

    let irq = interrupt::take!(QDEC);
    let config = qdec::Config::default();
    let mut rotary_enc = Qdec::new(p.QDEC, irq, p.P0_26, p.P0_27, config);

    info!("Turn rotary encoder!");

    loop {
        width += rotary_enc.read().await;
        info!("Value: {}", width);

        let im: ImageRawLE<Rgb565> = ImageRaw::new(
            include_bytes!("../../assets/ferris.raw"),
            width.max(0) as u32,
        );
        im.draw(&mut display).unwrap();
        display.flush().await.unwrap();
    }
}
