//! Demonstrate the use of a blocking `Delay` using the SYST (sysclock) timer.

#![deny(unsafe_code)]
#![allow(clippy::empty_loop)]
#![no_main]
#![no_std]

use embedded_hal::digital::v2::InputPin;
// Halt on panic
use panic_halt as _; // panic handler

use cortex_m_rt::entry;
use stm32f4xx_hal as hal;

use crate::hal::{pac, prelude::*};
use debounced_pin::{self, prelude::*, DebouncedInputPin};

#[entry]
fn main() -> ! {
    if let (Some(dp), Some(cp)) = (
        pac::Peripherals::take(),
        cortex_m::peripheral::Peripherals::take(),
    ) {
        // Set up the LED. On the Black Pill it's connected to pin PC13.
        let gpioc = dp.GPIOC.split();
        let mut led = gpioc.pc13.into_push_pull_output();

        // Set up User button. On the Black Pill it's connected to pin PA0
        let gpioa = dp.GPIOA.split();
        let user_button = gpioa.pa0.into_pull_up_input();
        let mut user_button = DebouncedInputPin::new(user_button, debounced_pin::ActiveLow);

        // Set up the system clock. We want to run at 48MHz for this one.
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();

        // Create a delay abstraction based on SysTick
        let mut delay = cp.SYST.delay(&clocks);

        let mut state = State::LedOff;

        loop {
            match state {
                State::LedOn => led.set_low(),
                State::LedOff => led.set_high(),
            }

            user_button.update().unwrap();

            if user_button.is_low().unwrap() {
                state.cycle();
            }
            delay.delay_ms(1_u32);
        }
    }

    loop {}
}

enum State {
    LedOn,
    LedOff,
}

impl State {
    fn cycle(&mut self) {
        use State::*;
        *self = match *self {
            LedOn => LedOff,
            LedOff => LedOn,
        }
    }
}
