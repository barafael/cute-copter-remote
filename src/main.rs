#![deny(unsafe_code)]
#![no_main]
#![cfg_attr(not(test), no_std)]

use cortex_m_rt::entry;
use cute_copter_config_proto::command::Interactive;
use heapless::Vec;
use nrf24_rs::config::{NrfConfig, PALevel, PayloadSize};
use nrf24_rs::Nrf24l01;
use panic_rtt_target as _;
use postcard::to_vec;
use rtt_target::{rprintln, rtt_init_print};
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::spi::Mode as SpiMode;
use stm32f1xx_hal::spi::Spi;
use stm32f1xx_hal::{adc, pac};

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

    let mut delay = cp.SYST.delay(&clocks);

    let mut gpioa = dp.GPIOA.split();
    let gpiob = dp.GPIOB.split();

    // Setup SPI
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

    // Setup analog pins for joysticks
    let mut adc1 = adc::Adc::adc1(dp.ADC1, clocks);
    let mut ch0 = gpioa.pa0.into_analog(&mut gpioa.crl);
    let mut ch1 = gpioa.pa1.into_analog(&mut gpioa.crl);
    let mut ch2 = gpioa.pa2.into_analog(&mut gpioa.crl);
    let mut ch3 = gpioa.pa3.into_analog(&mut gpioa.crl);

    let (chip_enable, ..) =
        stm32f1xx_hal::afio::MAPR::disable_jtag(&mut afio.mapr, gpioa.pa15, gpiob.pb3, gpiob.pb4);
    let chip_enable = chip_enable.into_push_pull_output(&mut gpioa.crh);

    let config = NrfConfig::default()
        .channel(8)
        .pa_level(PALevel::Low)
        .ack_payloads_enabled(true)
        // We will use a payload size the size of our message
        .payload_size(PayloadSize::Static(MESSAGE.len() as u8));

    // Initialize the nrf chip
    let mut nrf = Nrf24l01::new(spi, chip_enable, cs, &mut delay, config).unwrap();
    if !nrf.is_connected().unwrap() {
        panic!("Radio is not connected.");
    }
    nrf.open_writing_pipe(b"Node1").unwrap();
    nrf.stop_listening().unwrap();

    rprintln!("Starting tx loop");

    let mut data = Interactive::default();

    loop {
        data.throttle = adc1.read(&mut ch0).unwrap();
        data.roll = adc1.read(&mut ch1).unwrap();
        data.pitch = adc1.read(&mut ch2).unwrap();
        data.yaw = adc1.read(&mut ch3).unwrap();

        rprintln!("{:?}", data);

        let output: Vec<u8, 8> = to_vec(&data).unwrap();

        while let Err(e) = nrf.write(&mut delay, &output) {
            rprintln!("{:?}", e);
            delay.delay_ms(50u32);
        }

        delay.delay_ms(20u32);
    }
}
