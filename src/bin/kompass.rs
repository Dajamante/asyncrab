//! Connect SDA to P0.03, SCL to P0.04
//! $ DEFMT_LOG=info cargo rb simple

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use nrf_embassy as _; // global logger + panicking-behavior

use defmt::{info,Format};
use embassy::executor::Spawner;
use embassy::time::{Delay, Duration, Timer};
use embassy_nrf::twim::{self, Twim};
use embassy_nrf::{interrupt, Peripherals};
use hmc5983_async::*;

#[embassy::main]
async fn main(_spawner: Spawner, p: Peripherals) {
    let config = twim::Config::default();
    let irq = interrupt::take!(SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0);
    let i2c = Twim::new(p.TWISPI0, irq, p.P0_03, p.P0_04, config);

    let mut hmc = HMC5983::new_with_interface(hmc5983_async::interface::I2cInterface::new(i2c));
    hmc.init(&mut Delay).expect("mag_int init failed");

    loop {
        // get roll and pitch estimate
        if let Ok(temp) = hmc.get_temperature(){
            info!("acc: {:?}", temp);
        }
        // get roll and pitch estimate
        if let Ok(mag) = hmc.get_mag_vector(){
            info!("mag: {:?}", mag);
        }
        

        Timer::after(Duration::from_secs(10)).await;
    }
}