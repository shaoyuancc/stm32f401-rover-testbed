#![deny(unsafe_code)]
#![allow(clippy::empty_loop)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;
use panic_semihosting as _;
use stm32f4xx_hal as hal;

use crate::hal::{pac, prelude::*};

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

        // Set up the system clock. We want to run at 48MHz for this one.
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();

        // Create a delay abstraction based on SysTick
        let mut delay = cp.SYST.delay(&clocks);

        let gpiob = dp.GPIOB.split();
        let m1l1 = gpiob.pb5.into_push_pull_output();
        let m1l2 = gpiob.pb4.into_push_pull_output();
        let m2l1 = gpioa.pa15.into_push_pull_output();
        let m2l2 = gpioa.pa12.into_push_pull_output();

        let tim4_channels = gpiob.pb6.into_alternate();
        let m1pwm = dp.TIM4.pwm_hz(tim4_channels, 20.kHz(), &clocks).split();
        let max_duty = m1pwm.get_max_duty();

        let tim1_channels = gpioa.pa11.into_alternate();
        let m2pwm = dp.TIM1.pwm_hz(tim1_channels, 20.kHz(), &clocks).split();

        let mut motors = l298n::L298N::new(m1l1, m1l2, m1pwm, m2l1, m2l2, m2pwm);
        motors.a.set_duty(max_duty);
        motors.b.set_duty(max_duty);

        loop {
            motors.a.forward();
            // motors.b.forward();
            delay.delay_ms(3000_u32);
            motors.a.reverse();
            // motors.b.reverse();
            delay.delay_ms(3000_u32);
            motors.a.stop();
            motors.b.stop();
            delay.delay_ms(3000_u32);
        }
    }

    loop {}
}
