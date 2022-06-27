//! Draw Ferris the Rust mascot on an SSD1306 display

#![allow(clippy::empty_loop)]
#![no_std]
#![no_main]

use cortex_m_rt::ExceptionFrame;
use cortex_m_rt::{entry, exception};
use embedded_graphics::{
    image::Image,
    image::ImageRaw,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Alignment, Text},
};
use hal::gpio::{Alternate, OpenDrain, Pin};
use hal::i2c::I2c;
use hal::pac::I2C1;
use panic_halt as _;
use ssd1306::mode::BufferedGraphicsMode;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use stm32f4xx_hal as hal;

use crate::hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    if let (Some(dp), Some(_cp)) = (
        pac::Peripherals::take(),
        cortex_m::peripheral::Peripherals::take(),
    ) {
        // Set up the system clock. We want to run at 48MHz for this one.
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();

        // Set up I2C - SCL is PB8 and SDA is PB9; they are set to Alternate Function 4
        // as per the STM32F446xC/E datasheet page 60. Pin assignment as per the Nucleo-F446 board.
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
        // let i2c = I2c::new(dp.I2C1, (scl, sda), 400.kHz(), &clocks);
        // or
        let i2c = dp.I2C1.i2c((scl, sda), 400.kHz(), &clocks);

        // Set up button
        let gpioa = dp.GPIOA.split();
        let btn = gpioa.pa0.into_pull_up_input();

        // Set up the display
        let interface = I2CDisplayInterface::new(i2c);
        let mut disp = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        disp.init().unwrap();
        disp.flush().unwrap();

        // Create a new character style
        let style: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_10X20, BinaryColor::On);

        // Create a text at position (64, 32) and draw it using the previously defined style
        let welcome_text: Text<MonoTextStyle<BinaryColor>> = Text::with_alignment(
            "Hello\nShao Yuan",
            Point::new(64, 32),
            style,
            Alignment::Center,
        );

        let goodbye_text = Text::with_alignment(
            "Goodbye\nSee you soon!",
            Point::new(64, 32),
            style,
            Alignment::Center,
        );

        // Display the rustacean
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
                state.cycle();
                was_pressed = true;

                use State::*;
                match state {
                    Image => {
                        show_drawable(&image, &mut disp);
                    }
                    Text1 => {
                        show_drawable(&welcome_text, &mut disp);
                    }
                    Text2 => {
                        show_drawable(&goodbye_text, &mut disp);
                    }
                };
            } else if !is_pressed {
                was_pressed = false;
            }
        }
    }

    loop {}
}

fn show_drawable(
    item: &impl Drawable<Color = BinaryColor>,
    disp: &mut Ssd1306<
        I2CInterface<
            I2c<
                I2C1,
                (
                    Pin<'B', 8, Alternate<4, OpenDrain>>,
                    Pin<'B', 9, Alternate<4, OpenDrain>>,
                ),
            >,
        >,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
) {
    disp.clear();
    item.draw(disp).unwrap();
    disp.flush().unwrap();
}
enum State {
    Image,
    Text1,
    Text2,
}

impl State {
    fn cycle(&mut self) {
        use State::*;
        *self = match *self {
            Image => Text1,
            Text1 => Text2,
            Text2 => Image,
        }
    }
}

#[exception]
unsafe fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
