// $ cargo rb buttons
#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use nrf_embassy::{self as _, println}; // global logger + panicking-behavior + memory layout

use defmt::{info, Format};
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::channel::mpmc::{Channel, Sender};
use embassy::executor::Spawner;
use embassy::time::{Delay, Duration, Timer};
use embassy_nrf::gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull};
use embassy_nrf::{interrupt, spim, Peripherals};

use embedded_graphics::{image::Image, pixelcolor::Rgb565, prelude::*};
use embedded_hal_async::spi::ExclusiveDevice;
use st7735_embassy::{self, ST7735};
use tinybmp::Bmp;

static CHANNEL: Channel<ThreadModeRawMutex, ButtonEvent, 1> = Channel::new();

// Dimensions of the character
const FERRIS_LENGTH: i32 = 86;
const FERRIS_HEIGHT: i32 = 64;
const SCREEN_LENGTH: i32 = 160;
const SCREEN_HEIGHT: i32 = 128;
// This task awaits for the button to go high and low,
// and debounces by waiting 25 ms.
#[embassy::task(pool_size = 4)]
async fn button_listener(
    sender: Sender<'static, ThreadModeRawMutex, ButtonEvent, 1>,
    id: Button,
    mut pin: Input<'static, AnyPin>,
) {
    loop {
        pin.wait_for_low().await;
        Timer::after(Duration::from_millis(25)).await; // Debounce
        if pin.is_low() {
            let _ = sender.send(ButtonEvent::Pressed(id)).await;
            pin.wait_for_high().await;
            let _ = sender.send(ButtonEvent::Released(id)).await;
        }
    }
}

#[embassy::main]
async fn main(spawner: Spawner, p: Peripherals) {
    // Channel
    let sender = CHANNEL.sender();
    let receiver = CHANNEL.receiver();
    // Buttons configuration
    let btn1 = Input::new(p.P0_11.degrade(), Pull::Up);
    let btn2 = Input::new(p.P0_12.degrade(), Pull::Up);
    let btn3 = Input::new(p.P0_24.degrade(), Pull::Up);
    let btn4 = Input::new(p.P0_25.degrade(), Pull::Up);

    spawner
        .spawn(button_listener(sender.clone(), Button::Left, btn1))
        .unwrap();
    spawner
        .spawn(button_listener(sender.clone(), Button::Right, btn2))
        .unwrap();
    spawner
        .spawn(button_listener(sender.clone(), Button::Up, btn3))
        .unwrap();
    spawner
        .spawn(button_listener(sender.clone(), Button::Down, btn4))
        .unwrap();

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

    let raw_image_front: Bmp<Rgb565> =
        Bmp::from_slice(include_bytes!("../../assets/ferris_fr.bmp")).unwrap();
    let mut start_point = Point { x: 32, y: 24 };
    let mut image = Image::new(&raw_image_front, start_point);

    let raw_image_back: Bmp<Rgb565> =
        Bmp::from_slice(include_bytes!("../../assets/ferris_back.bmp")).unwrap();

    image.draw(&mut display).unwrap();
    display.flush().await.unwrap();

    // LED is set to max, but can be modulated with pwm to change backlight brightness
    let mut backlight = Output::new(p.P0_03, Level::High, OutputDrive::Standard);

    backlight.set_high();
    let mut is_turned = false;

    loop {
        if let event = receiver.recv().await {
            match event {
                ButtonEvent::Pressed(id) => {
                    info!("Btn {:#?} pressed", id);
                    match id {
                        Button::Right => {
                            start_point.x = if start_point.x + 10 > SCREEN_LENGTH {
                                -FERRIS_LENGTH
                            } else {
                                start_point.x + 10
                            }
                        }
                        Button::Left => {
                            start_point.x = if start_point.x - 10 < -FERRIS_LENGTH {
                                160
                            } else {
                                start_point.x - 10
                            };
                        }
                        Button::Up => {
                            is_turned = true;

                            start_point.y = if start_point.y - 10 < -FERRIS_HEIGHT {
                                128
                            } else {
                                start_point.y - 10
                            };
                        }
                        Button::Down => {
                            is_turned = false;
                            start_point.y = if start_point.y + 10 > SCREEN_HEIGHT {
                                -FERRIS_HEIGHT
                            } else {
                                start_point.y + 10
                            };
                        }
                    }
                    if is_turned {
                        image = Image::new(&raw_image_back, start_point);
                    } else {
                        image = Image::new(&raw_image_front, start_point);
                    }

                    display.clear(Rgb565::BLACK).unwrap();
                    image.draw(&mut display).unwrap();
                    display.flush().await.unwrap();
                }
                ButtonEvent::Released(id) => {
                    info!("Btn {:#?} released", id);
                }
            }
        };
    }
}

#[derive(Clone, Copy, Format)]
enum ButtonEvent {
    Pressed(Button),
    Released(Button),
}

#[derive(Clone, Copy, Format)]
enum Button {
    Up,
    Left,
    Down,
    Right,
}
