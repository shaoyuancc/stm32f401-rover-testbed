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
        let mut m1l1 = gpiob.pb5.into_push_pull_output();
        let mut m1l2 = gpiob.pb4.into_push_pull_output();
        let mut m2l1 = gpioa.pa15.into_push_pull_output();
        let mut m2l2 = gpioa.pa12.into_push_pull_output();

        let tim4_channels = gpiob.pb6.into_alternate();
        let mut m1pwm = dp.TIM4.pwm_hz(tim4_channels, 20.kHz(), &clocks).split();

        let tim1_channels = gpioa.pa11.into_alternate();
        let mut m2pwm = dp.TIM1.pwm_hz(tim1_channels, 20.kHz(), &clocks).split();

        let max_duty = m1pwm.get_max_duty();
        m1pwm.set_duty(max_duty * 2 / 3);
        m1pwm.enable();

        m2pwm.set_duty(max_duty * 2 / 3);
        m2pwm.enable();

        loop {
            m1pwm.set_duty(max_duty * 2 / 3);
            m2pwm.set_duty(max_duty * 2 / 3);
            m1l1.set_high();
            m1l2.set_low();
            m2l1.set_high();
            m2l2.set_low();
            delay.delay_ms(3000_u32);
            m1pwm.set_duty(max_duty);
            m2pwm.set_duty(max_duty);
            m1l1.set_low();
            m1l2.set_high();
            m2l1.set_low();
            m2l2.set_high();
            delay.delay_ms(3000_u32);
            m1l1.set_low();
            m1l2.set_low();
            m2l1.set_low();
            m2l2.set_low();
            delay.delay_ms(3000_u32);
        }
    }

    loop {}
}
