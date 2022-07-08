#![no_main]
#![no_std]

use panic_semihosting as _;
use rtic::app;

#[app(device = hal::pac, peripherals = true)]
mod app {
    use cortex_m_semihosting::hprintln;
    use hal::prelude::*;
    use stm32f4xx_hal as hal;

    #[shared]
    struct Shared {
        led: hal::gpio::gpioc::PC13<hal::gpio::Output<hal::gpio::PushPull>>,
        delay: hal::timer::SysDelay,
    }

    #[local]
    struct Local {
        btn: hal::gpio::gpioa::PA0<hal::gpio::Input>,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let dp = ctx.device;
        let cp = ctx.core;
        let rcc = dp.RCC.constrain();
        // let mut flash = dp.FLASH.constrain();
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();
        let delay = cp.SYST.delay(&clocks);

        let mut exti = dp.EXTI;
        let mut syscfg = dp.SYSCFG.constrain();

        let gpioc = dp.GPIOC.split();
        let mut led = gpioc.pc13.into_push_pull_output();
        led.set_high();

        let gpioa = dp.GPIOA.split();
        let mut btn = gpioa.pa0.into_pull_up_input();
        btn.make_interrupt_source(&mut syscfg);
        btn.trigger_on_edge(&mut exti, hal::gpio::Edge::Falling);
        btn.enable_interrupt(&mut exti);
        (Shared { led, delay }, Local { btn }, init::Monotonics())
    }

    #[task(binds=EXTI0, shared = [led, delay], local = [btn])]
    fn exti0_event(ctx: exti0_event::Context) {
        let delay = ctx.shared.delay;
        let led = ctx.shared.led;
        let btn = ctx.local.btn;

        hprintln!("Button!").unwrap();
        (led, delay).lock(|led, delay| {
            led.toggle();
            delay.delay_ms(3000_u16);
            led.toggle();
            btn.clear_interrupt_pending_bit();
        });
    }

    #[idle(shared= [led, delay])]
    fn idle(ctx: idle::Context) -> ! {
        let mut delay = ctx.shared.delay;
        let mut led = ctx.shared.led;

        let ms = 500_u16;
        loop {
            hprintln!("Blink!").unwrap();
            led.lock(|led| {
                led.toggle();
            });
            delay.lock(|delay| delay.delay_ms(ms));
        }
    }
}
