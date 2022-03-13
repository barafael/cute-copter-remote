#![deny(unsafe_code)]
#![no_main]
#![cfg_attr(not(test), no_std)]

use nrf24_rs::config::{DataPipe, NrfConfig, PALevel, PayloadSize};
use nrf24_rs::Nrf24l01;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};
use stm32f1xx_hal::pac;
use stm32f1xx_hal::prelude::*;

mod error;
use cortex_m_rt::entry;
use stm32f1xx_hal::spi::Mode as SpiMode;
use stm32f1xx_hal::spi::Spi;

pub const MODE: SpiMode = nrf24_rs::SPI_MODE;
const MESSAGE: &[u8; 17] = b"Here's a message!";

#[entry]
fn main() -> ! {
    // Setup clocks
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain();

    rtt_init_print!();
    rprintln!("init");

    let clocks = rcc
        .cfgr
        .sysclk(72.MHz())
        .pclk1(48.MHz())
        .freeze(&mut flash.acr);

    // Setup LED

    let mut gpioa = dp.GPIOA.split();
    let gpiob = dp.GPIOB.split();

    let mut delay = cp.SYST.delay(&clocks);

    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6;
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let cs = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);

    let spi = Spi::spi1(
        dp.SPI1,
        (sck, miso, mosi),
        &mut afio.mapr,
        MODE,
        1.MHz(),
        clocks,
    );

    let (chip_enable, ..) =
        stm32f1xx_hal::afio::MAPR::disable_jtag(&mut afio.mapr, gpioa.pa15, gpiob.pb3, gpiob.pb4);
    let chip_enable = chip_enable.into_push_pull_output(&mut gpioa.crh);

    let config = NrfConfig::default()
        .channel(8)
        .pa_level(PALevel::Min)
        // We will use a payload size the size of our message
        .payload_size(PayloadSize::Static(MESSAGE.len() as u8));

    // Initialize the chip
    let mut nrf = Nrf24l01::new(spi, chip_enable, cs, &mut delay, config).unwrap();
    if !nrf.is_connected().unwrap() {
        panic!("Chip is not connected.");
    }
    nrf.open_reading_pipe(DataPipe::DP0, b"Node1").unwrap();

    loop {
        while !nrf.data_available().unwrap() {
            delay.delay_ms(50u32);
        }
        let mut buffer = [0; MESSAGE.len()];
        nrf.read(&mut buffer).unwrap();
        rprintln!("{:?}", buffer);
    }
}
