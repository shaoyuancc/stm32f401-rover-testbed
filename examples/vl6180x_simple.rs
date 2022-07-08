//! Draw Ferris the Rust mascot on an SSD1306 display

#![allow(clippy::empty_loop)]
#![no_std]
#![no_main]

use core::cell::RefCell;

use crate::hal::{pac, prelude::*};
use cortex_m::interrupt::Mutex;
use cortex_m_rt::ExceptionFrame;
use cortex_m_rt::{entry, exception};
use cortex_m_semihosting::hprintln;
use embedded_graphics::{
    image::Image,
    image::ImageRaw,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use hal::gpio::{Alternate, OpenDrain, Pin};
use hal::i2c::I2c;
use hal::pac::I2C1;
use heapless::String;
use panic_semihosting as _;
use shared_bus::{self, I2cProxy};
use ssd1306::mode::BufferedGraphicsMode;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use stm32f4xx_hal as hal;
use vl6180x::RangeContinuousMode;
use vl6180x::VL6180X;

type I2cType = I2c<
    I2C1,
    (
        Pin<'B', 8, Alternate<4, OpenDrain>>,
        Pin<'B', 9, Alternate<4, OpenDrain>>,
    ),
>;

type DispType<'a> = Ssd1306<
    I2CInterface<I2cProxy<'a, Mutex<RefCell<I2cType>>>>,
    DisplaySize128x64,
    BufferedGraphicsMode<DisplaySize128x64>,
>;

type TofRangeContinuousModeType<'a> =
    VL6180X<RangeContinuousMode, I2cProxy<'a, Mutex<RefCell<I2cType>>>>;

#[entry]
fn main() -> ! {
    if let (Some(dp), Some(cp)) = (
        pac::Peripherals::take(),
        cortex_m::peripheral::Peripherals::take(),
    ) {
        // Set up the system clock. We want to run at 48MHz for this one.
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();

        // Create a delay abstraction based on SysTick
        let mut delay = cp.SYST.delay(&clocks);

        // Set up the LED. On the Black Pill it's connected to pin PC13.
        let gpioc = dp.GPIOC.split();
        let mut led = gpioc.pc13.into_push_pull_output();
        led.set_high();

        // Set up I2C - SCL is PB8 and SDA is PB9; they are set to Alternate Function 4
        let gpiob = dp.GPIOB.split();
        let scl = gpiob
            .pb8
            .into_alternate()
            .internal_pull_up(true)
            .set_open_drain();
        let sda = gpiob
            .pb9
            .into_alternate()
            .internal_pull_up(true)
            .set_open_drain();
        let i2c = dp.I2C1.i2c((scl, sda), 400.kHz(), &clocks);

        // Set up shared I2C bus (single task/thread)
        let bus: &'static _ = shared_bus::new_cortexm!(I2cType = i2c).unwrap();

        // Set up button
        let gpioa = dp.GPIOA.split();
        let btn = gpioa.pa0.into_pull_up_input();

        // To create sensor with default configuration:
        let mut tof_1 = vl6180x::VL6180X::new(bus.acquire_i2c())
            .expect("vl")
            .start_range_continuous_mode()
            .expect("start range continuous");
        // Set up the display
        let interface = I2CDisplayInterface::new(bus.acquire_i2c());
        let mut disp = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        disp.init().unwrap();
        disp.flush().unwrap();

        let mut was_pressed = btn.is_low();

        // This runs continuously, as fast as possible
        loop {
            let is_pressed = btn.is_low();
            if !was_pressed && is_pressed {
                // let _tof = tof_1.stop_range_continuous().expect("stop continuous");
                was_pressed = true;
            } else if !is_pressed {
                was_pressed = false;
            }
            match tof_1.read_range_mm_blocking() {
                Ok(range) => {
                    let mut text: String<50> = String::from("Range Continuous Poll\n");
                    text.push_str(&String::<4>::from(range)).unwrap();
                    text.push_str("mm").unwrap();

                    show_text(&text, &mut disp);
                }
                Err(e) => hprintln!("Error reading TOF sensor Continuous! {:?}", e).unwrap(),
            }
            delay.delay_ms(98_u32);
        }
    }

    loop {}
}

const WELCOME_TEXT: &str = "VL6180X\nSingle Test Suite";
const END_TEXT: &str = "Goodbye\nSee you soon!";

fn show_text(text: &str, disp: &mut DispType) {
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let disp_text: Text<MonoTextStyle<BinaryColor>> =
        Text::with_alignment(text, Point::new(64, 32), style, Alignment::Center);
    show_drawable(&disp_text, disp);
}

fn show_drawable(item: &impl Drawable<Color = BinaryColor>, disp: &mut DispType) {
    disp.clear();
    item.draw(disp).unwrap();
    disp.flush().unwrap();
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
