//! Draw Ferris the Rust mascot on an SSD1306 display

#![allow(clippy::empty_loop)]
#![no_std]
#![no_main]

use core::cell::RefCell;

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
use vl6180x::mode::DynamicMode;
use vl6180x::VL6180X;

use crate::hal::{pac, prelude::*};

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

type TofDynamicModeType<'a> = VL6180X<DynamicMode, I2cProxy<'a, Mutex<RefCell<I2cType>>>>;

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

        // Set up XShut pin
        let mut xshut = gpiob.pb13.into_push_pull_output();
        led.set_low();
        xshut.set_low();
        delay.delay_ms(200_u32);
        led.set_high();
        xshut.set_high();
        delay.delay_ms(200_u32);
        // Set up TOF distance sensor
        let tof_config = vl6180x::Config::new();
        // To create sensor with default configuration:
        let mut tof_1: TofDynamicModeType =
            vl6180x::VL6180X::with_config(bus.acquire_i2c(), &tof_config).expect("vl");

        // Set up the display
        let interface = I2CDisplayInterface::new(bus.acquire_i2c());
        let mut disp = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        disp.init().unwrap();
        disp.flush().unwrap();

        // Create image rustacean
        let raw_image: ImageRaw<BinaryColor> =
            ImageRaw::new(include_bytes!("../examples/ssd1306-image.data"), 128);
        let image = Image::new(&raw_image, Point::zero());
        image.draw(&mut disp).unwrap();
        disp.flush().unwrap();

        // Set up state for the loop
        let mut state = State::Image;
        let mut was_pressed = btn.is_low();

        // This runs continuously, as fast as possible
        loop {
            let is_pressed = btn.is_low();
            if !was_pressed && is_pressed {
                use State::*;
                // On exiting state
                match state {
                    RangeContinuousPoll => tof_1
                        .try_stop_range_continuous_mode()
                        .expect("stop range continuous"),
                    AmbientContinuousPoll => tof_1
                        .try_stop_ambient_continuous_mode()
                        .expect("stop ambeint continuous"),
                    _ => (),
                };

                state.cycle();
                was_pressed = true;

                // On first entering state
                match state {
                    Image => show_drawable(&image, &mut disp),
                    WelcomeText => show_text(&WELCOME_TEXT, &mut disp),
                    RangeContinuousPoll => tof_1
                        .try_start_range_continuous_mode()
                        .expect("start range cont"),
                    AmbientContinuousPoll => tof_1
                        .try_start_ambient_continuous_mode()
                        .expect("start ambient cont"),
                    // Single => (),
                    // AddressCycle => {
                    //     for i in 0_u8..255 {
                    //         if i == 60 {
                    //             continue; // Skip the I2C address of the OLED
                    //         }
                    //         match gyul53l0x.set_address(i) {
                    //             Ok(()) => {
                    //                 if !(i > 0x07 && i < 0x78) {
                    //                     hprintln!(
                    //                         "Failed cycle test! Invalid address was accepted"
                    //                     )
                    //                     .unwrap();
                    //                 }
                    //             }
                    //             Err(_e) => {
                    //                 if i > 0x07 && i < 0x78 {
                    //                     hprintln!(
                    //                         "Failed cycle test! Valid address {} did not work",
                    //                         i
                    //                     )
                    //                     .unwrap();
                    //                 } else {
                    //                     hprintln!(
                    //                         "correctly received error for invalid adddress {}",
                    //                         i
                    //                     )
                    //                     .unwrap();
                    //                     let mut text: String<50> = String::from(
                    //                         "Correctly\nreceived error:\nInvalid address ",
                    //                     );
                    //                     text.push_str(&String::<3>::from(i)).unwrap();
                    //                     show_text(&text, &mut disp);
                    //                     continue;
                    //                 }
                    //             }
                    //         }
                    //         match gyul53l0x.read_range_single_millimeters_blocking() {
                    //             Ok(range) => {
                    //                 let mut text: String<20> = String::from("Address: ");
                    //                 text.push_str(&String::<3>::from(i)).unwrap();
                    //                 text.push_str("\n").unwrap();
                    //                 text.push_str(&String::<4>::from(range)).unwrap();
                    //                 text.push_str("mm").unwrap();

                    //                 show_text(&text, &mut disp);
                    //             }
                    //             Err(_e) => {
                    //                 hprintln!("Error reading TOF sensor at address {}!", i).unwrap()
                    //             }
                    //         };
                    //         delay.delay_ms(50_u32);
                    //     }
                    // }
                    // WhoAmI => {
                    //     let addr = gyul53l0x.who_am_i().expect("who am i");
                    //     let mut text: String<16> = String::from("Who Am I?\n");
                    //     text.push_str(&String::<4>::from(addr)).expect("addr conv");

                    //     show_text(&text, &mut disp);
                    // }
                    EndText => show_text(&END_TEXT, &mut disp),
                    _ => (),
                };
            } else if !is_pressed {
                was_pressed = false;
            }
            // While in state
            use State::*;
            match state {
                RangeContinuousPoll => match tof_1.try_read_range_blocking_mm() {
                    Ok(range) => {
                        let mut text: String<50> = String::from("Range Continuous\nPoll ");
                        text.push_str(&String::<4>::from(range)).unwrap();
                        text.push_str("mm").unwrap();

                        show_text(&text, &mut disp);
                    }
                    Err(e) => hprintln!("Error reading TOF sensor Continuous! {:?}", e).unwrap(),
                },
                RangeSinglePoll => match tof_1.try_poll_range_single_blocking_mm() {
                    Ok(range) => {
                        let mut text: String<50> = String::from("Range Single\nPoll ");
                        text.push_str(&String::<4>::from(range)).unwrap();
                        text.push_str("mm").unwrap();

                        show_text(&text, &mut disp);
                    }
                    Err(e) => hprintln!("Error reading TOF sensor Single Poll! {:?}", e).unwrap(),
                },
                AmbientContinuousPoll => match tof_1.try_read_ambient_blocking() {
                    Ok(range) => {
                        let mut text: String<50> = String::from("Ambient Continuous\nPoll ");
                        text.push_str(&String::<4>::from(range)).unwrap();
                        text.push_str("mm").unwrap();

                        show_text(&text, &mut disp);
                    }
                    Err(e) => hprintln!("Error reading TOF sensor Continuous! {:?}", e).unwrap(),
                },
                AmbientSinglePoll => match tof_1.try_poll_ambient_single_blocking() {
                    Ok(range) => {
                        let mut text: String<50> = String::from("Ambient Single\nPoll ");
                        text.push_str(&String::<4>::from(range)).unwrap();
                        text.push_str("mm").unwrap();

                        show_text(&text, &mut disp);
                    }
                    Err(e) => hprintln!("Error reading TOF sensor Single Poll! {:?}", e).unwrap(),
                },
                // AddressCycle => show_text("Done Cycling\nAddresses", &mut disp),
                _ => (),
            };
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

enum State {
    Image,
    WelcomeText,
    RangeContinuousPoll,
    RangeContinuousHostPoll,
    RangeContinuousInterrupt,
    RangeSinglePoll,
    RangeSingleHostPoll,
    RangeSingleInterrupt,
    AmbientContinuousPoll,
    AmbientSinglePoll,
    AddressCycle,
    WhoAmI,
    EndText,
}

impl State {
    fn cycle(&mut self) {
        use State::*;
        *self = match *self {
            Image => WelcomeText,
            WelcomeText => RangeContinuousPoll,
            RangeContinuousPoll => RangeSinglePoll,
            // RangeContinuousHostPoll,
            // RangeContinuousInterrupt,
            RangeSinglePoll => AmbientContinuousPoll,
            AmbientContinuousPoll => AmbientSinglePoll,
            AmbientSinglePoll => EndText,
            // RangeSingleHostPoll,
            // RangeSingleInterrupt,
            // AddressCycle => WhoAmI,
            // WhoAmI => EndText,
            EndText => Image,
            _ => Image,
        }
    }
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
