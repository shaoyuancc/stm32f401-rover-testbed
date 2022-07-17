#![no_main]
#![no_std]

use panic_semihosting as _;
use rtic::app;

#[app(device = hal::pac, peripherals = true)]
mod app {
    use cortex_m_semihosting::hprintln;
    use hal::prelude::*;
    use stm32f4xx_hal as hal;

    type I2c = hal::i2c::I2c<
        you_must_enable_the_rt_feature_for_the_pac_in_your_cargo_toml::I2C1,
        (
            hal::gpio::gpiob::PB8<hal::gpio::Alternate<4, hal::gpio::OpenDrain>>,
            hal::gpio::gpiob::PB9<hal::gpio::Alternate<4, hal::gpio::OpenDrain>>,
        ),
    >;
    type I2cProxy = shared_bus::I2cProxy<'static, shared_bus::AtomicCheckMutex<I2c>>;

    type Vl6180xType = vl6180x::VL6180X<vl6180x::RangeContinuousMode, I2cProxy>;

    type TofBRType = vl6180x::VL6180XwPins<
        vl6180x::RangeContinuousMode,
        I2cProxy,
        hal::gpio::gpioc::PC15<hal::gpio::Output>,
        hal::gpio::gpioc::PC14<hal::gpio::Input>,
    >;

    type TofFRType = vl6180x::VL6180XwPins<
        vl6180x::RangeContinuousMode,
        I2cProxy,
        hal::gpio::gpioa::PA2<hal::gpio::Output>,
        hal::gpio::gpioa::PA1<hal::gpio::Input>,
    >;

    type TofFLType = vl6180x::VL6180XwPins<
        vl6180x::RangeContinuousMode,
        I2cProxy,
        hal::gpio::gpioa::PA5<hal::gpio::Output>,
        hal::gpio::gpioa::PA4<hal::gpio::Input>,
    >;

    type TofBLType = vl6180x::VL6180XwPins<
        vl6180x::RangeContinuousMode,
        I2cProxy,
        hal::gpio::gpiob::PB1<hal::gpio::Output>,
        hal::gpio::gpiob::PB0<hal::gpio::Input>,
    >;

    type MotorsType = l298n::L298N<
        hal::gpio::gpiob::PB5<hal::gpio::Output<hal::gpio::PushPull>>,
        hal::gpio::gpiob::PB4<hal::gpio::Output<hal::gpio::PushPull>>,
        hal::gpio::gpioa::PA15<hal::gpio::Output<hal::gpio::PushPull>>,
        hal::gpio::gpioa::PA12<hal::gpio::Output<hal::gpio::PushPull>>,
        hal::timer::PwmChannel<hal::pac::TIM4, 0>,
        hal::timer::PwmChannel<hal::pac::TIM1, 3>,
    >;

    pub struct I2cDevices {
        tof_br: TofBRType,
        tof_fr: TofFRType,
        tof_fl: TofFLType,
        tof_bl: TofBLType,
    }

    #[derive(Debug)]
    pub struct Cliffs {
        br: bool,
        fr: bool,
        fl: bool,
        bl: bool,
    }
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub enum Heading {
        Forward,
        Reverse,
    }
    impl Heading {
        fn toggle(self) -> Heading {
            match self {
                Heading::Forward => Heading::Reverse,
                Heading::Reverse => Heading::Forward,
            }
        }
    }
    #[derive(Debug, Copy, Clone, PartialEq)]
    pub enum TurnDirection {
        Left,
        Right,
    }

    #[derive(Debug, Copy, Clone, PartialEq)]
    pub enum Command {
        Advance,
        PreTurn,
        Turn,
        Standby,
    }

    #[derive(Debug, Copy, Clone, PartialEq)]
    pub struct DriveState {
        command: Command,
        heading: Heading,
        turn_direction: TurnDirection,
        pre_turn_count: u32,
        turn_count: u32,
    }

    const CLIFF_THRESHOLD: u16 = 20;
    const PRE_TURN_COUNT_THRESHOLD: u32 = 40000;
    const TURN_COUNT_THRESHOLD: u32 = 60000;

    #[shared]
    struct Shared {
        i2c_devices: I2cDevices,
        motors: MotorsType,
        cliffs: Cliffs,
        led: hal::gpio::gpioc::PC13<hal::gpio::Output<hal::gpio::PushPull>>,
    }

    #[local]
    struct Local {
        drive_state: DriveState,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let dp = ctx.device;
        let cp = ctx.core;
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(48.MHz()).freeze();
        let mut delay = cp.SYST.delay(&clocks);
        let mut exti = dp.EXTI;
        let mut syscfg = dp.SYSCFG.constrain();

        let gpioa = dp.GPIOA.split();
        let gpiob = dp.GPIOB.split();
        let gpioc = dp.GPIOC.split();

        // Set up led
        let mut led = gpioc.pc13.into_push_pull_output();
        led.set_high();

        // Create the shared-bus I2C manager.
        let bus_manager: &'static _ = {
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

            shared_bus::new_atomic_check!(I2c = i2c).unwrap()
        };

        let mut tof_config = vl6180x::Config::new();
        tof_config.set_range_interrupt_mode(vl6180x::RangeInterruptMode::NewSampleReady);
        tof_config.set_range_max_convergence_time(10).expect("rmc");
        tof_config
            .set_range_inter_measurement_period(20)
            .expect("rimp");

        // Set up x_shut pins
        let mut x_shut_br = gpioc.pc15.into_push_pull_output();
        let mut x_shut_fr = gpioa.pa2.into_push_pull_output();
        let mut x_shut_fl = gpioa.pa5.into_push_pull_output();
        let mut x_shut_bl = gpiob.pb1.into_push_pull_output();

        x_shut_br.set_low();
        x_shut_fr.set_low();
        x_shut_fl.set_low();
        x_shut_bl.set_low();

        // Set up interrupt pins
        let mut int_br = gpioc.pc14.into_pull_up_input();
        int_br.make_interrupt_source(&mut syscfg);
        int_br.trigger_on_edge(&mut exti, hal::gpio::Edge::Rising);
        int_br.enable_interrupt(&mut exti);
        let mut int_fr = gpioa.pa1.into_pull_up_input();
        int_fr.make_interrupt_source(&mut syscfg);
        int_fr.trigger_on_edge(&mut exti, hal::gpio::Edge::Rising);
        int_fr.enable_interrupt(&mut exti);
        let mut int_fl = gpioa.pa4.into_pull_up_input();
        int_fl.make_interrupt_source(&mut syscfg);
        int_fl.trigger_on_edge(&mut exti, hal::gpio::Edge::Rising);
        int_fl.enable_interrupt(&mut exti);
        let mut int_bl = gpiob.pb0.into_pull_up_input();
        int_bl.make_interrupt_source(&mut syscfg);
        int_bl.trigger_on_edge(&mut exti, hal::gpio::Edge::Rising);
        int_bl.enable_interrupt(&mut exti);

        // Set up vl6180x's
        x_shut_br.set_high();
        delay.delay_ms(50_u8);
        let mut vl6180x_br =
            vl6180x::VL6180X::with_config(bus_manager.acquire_i2c(), &tof_config).expect("vl1");
        vl6180x_br.change_i2c_address(10).expect("sa1");

        x_shut_fr.set_high();
        delay.delay_ms(50_u8);
        let mut vl6180x_fr =
            vl6180x::VL6180X::with_config(bus_manager.acquire_i2c(), &tof_config).expect("vl2");
        vl6180x_fr.change_i2c_address(11).expect("sa2");

        x_shut_fl.set_high();
        delay.delay_ms(50_u8);
        let mut vl6180x_fl =
            vl6180x::VL6180X::with_config(bus_manager.acquire_i2c(), &tof_config).expect("vl3");
        vl6180x_fl.change_i2c_address(12).expect("sa3");

        x_shut_bl.set_high();
        delay.delay_ms(50_u8);
        let mut vl6180x_bl =
            vl6180x::VL6180X::with_config(bus_manager.acquire_i2c(), &tof_config).expect("vl4");
        vl6180x_bl.change_i2c_address(13).expect("sa4");

        // Start continuous range measurement
        let vl6180x_br: Vl6180xType = vl6180x_br.start_range_continuous_mode().expect("ct1");
        let vl6180x_fr: Vl6180xType = vl6180x_fr.start_range_continuous_mode().expect("ct2");
        let vl6180x_fl: Vl6180xType = vl6180x_fl.start_range_continuous_mode().expect("ct3");
        let vl6180x_bl: Vl6180xType = vl6180x_bl.start_range_continuous_mode().expect("ct4");

        // Compose them into objects
        let tof_br: TofBRType = vl6180x::VL6180XwPins {
            vl6180x: vl6180x_br,
            x_shutdown_pin: x_shut_br,
            interrupt_pin: int_br,
        };
        let tof_fr: TofFRType = vl6180x::VL6180XwPins {
            vl6180x: vl6180x_fr,
            x_shutdown_pin: x_shut_fr,
            interrupt_pin: int_fr,
        };
        let tof_fl: TofFLType = vl6180x::VL6180XwPins {
            vl6180x: vl6180x_fl,
            x_shutdown_pin: x_shut_fl,
            interrupt_pin: int_fl,
        };
        let tof_bl: TofBLType = vl6180x::VL6180XwPins {
            vl6180x: vl6180x_bl,
            x_shutdown_pin: x_shut_bl,
            interrupt_pin: int_bl,
        };

        // Set up motor driver
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

        let i2c_devices = I2cDevices {
            tof_br,
            tof_fr,
            tof_fl,
            tof_bl,
        };

        let cliffs = Cliffs {
            br: true,
            fr: true,
            fl: true,
            bl: true,
        };

        let drive_state = DriveState {
            command: Command::Standby,
            heading: Heading::Forward,
            turn_direction: TurnDirection::Left,
            pre_turn_count: 0,
            turn_count: 0,
        };

        (
            Shared {
                i2c_devices,
                motors,
                cliffs,
                led,
            },
            Local { drive_state },
            init::Monotonics(),
        )
    }

    #[task(binds=EXTI15_10, shared = [cliffs, i2c_devices])]
    fn exti15_10event(ctx: exti15_10event::Context) {
        let cliffs = ctx.shared.cliffs;
        let i2c_devices = ctx.shared.i2c_devices;

        // hprintln!("-------- Interrupt! -------- (tof_br)").unwrap();
        (cliffs, i2c_devices).lock(|cliffs, i2c_devices| {
            match i2c_devices.tof_br.vl6180x.read_range_mm() {
                Ok(range) => {
                    cliffs.br = range > CLIFF_THRESHOLD;
                    // hprintln!("Range Read: {}mm", range).unwrap();
                }
                Err(_e) => (), //hprintln!("Error {:?}", e).unwrap(),
            };
            i2c_devices
                .tof_br
                .interrupt_pin
                .clear_interrupt_pending_bit();
            i2c_devices
                .tof_br
                .vl6180x
                .clear_all_interrupts()
                .expect("clrall");
        });
    }

    #[task(binds=EXTI1, shared = [cliffs, i2c_devices])]
    fn exti1_event(ctx: exti1_event::Context) {
        let cliffs = ctx.shared.cliffs;
        let i2c_devices = ctx.shared.i2c_devices;

        // hprintln!("-------- Interrupt! -------- (tof_fr)").unwrap();
        (cliffs, i2c_devices).lock(|cliffs, i2c_devices| {
            match i2c_devices.tof_fr.vl6180x.read_range_mm() {
                Ok(range) => {
                    cliffs.fr = range > CLIFF_THRESHOLD;
                    // hprintln!("Range Read: {}mm", range).unwrap();
                }
                Err(_e) => (), //hprintln!("Error {:?}", e).unwrap(),
            };
            i2c_devices
                .tof_fr
                .interrupt_pin
                .clear_interrupt_pending_bit();
            i2c_devices
                .tof_fr
                .vl6180x
                .clear_all_interrupts()
                .expect("clrall");
        });
    }

    #[task(binds=EXTI4, shared = [cliffs, i2c_devices])]
    fn exti4_event(ctx: exti4_event::Context) {
        let cliffs = ctx.shared.cliffs;
        let i2c_devices = ctx.shared.i2c_devices;

        // hprintln!("-------- Interrupt! -------- (tof_fl)").unwrap();
        (cliffs, i2c_devices).lock(|cliffs, i2c_devices| {
            match i2c_devices.tof_fl.vl6180x.read_range_mm() {
                Ok(range) => {
                    cliffs.fl = range > CLIFF_THRESHOLD;
                    // hprintln!("Range Read: {}mm", range).unwrap();
                }
                Err(_e) => (), //hprintln!("Error {:?}", e).unwrap(),
            };
            i2c_devices
                .tof_fl
                .interrupt_pin
                .clear_interrupt_pending_bit();
            i2c_devices
                .tof_fl
                .vl6180x
                .clear_all_interrupts()
                .expect("clrall");
        });
    }

    #[task(binds=EXTI0, shared = [cliffs, i2c_devices])]
    fn exti0_event(ctx: exti0_event::Context) {
        let cliffs = ctx.shared.cliffs;
        let i2c_devices = ctx.shared.i2c_devices;

        // hprintln!("-------- Interrupt! -------- (tof_bl)").unwrap();
        (cliffs, i2c_devices).lock(|cliffs, i2c_devices| {
            match i2c_devices.tof_bl.vl6180x.read_range_mm() {
                Ok(range) => {
                    cliffs.bl = range > CLIFF_THRESHOLD;
                    // hprintln!("Range Read: {}mm", range).unwrap();
                }
                Err(_e) => (), //hprintln!("Error {:?}", e).unwrap(),
            };
            i2c_devices
                .tof_bl
                .interrupt_pin
                .clear_interrupt_pending_bit();
            i2c_devices
                .tof_bl
                .vl6180x
                .clear_all_interrupts()
                .expect("clrall");
        });
    }

    #[idle(shared = [cliffs, motors], local=[drive_state])]
    fn idle(ctx: idle::Context) -> ! {
        let mut cliffs = ctx.shared.cliffs;
        let mut motors = ctx.shared.motors;
        let drive_state = ctx.local.drive_state;

        loop {
            cliffs.lock(|cliffs| {
                // hprintln!("{:?}", cliffs).unwrap();
                if !cliffs.br && !cliffs.fr && !cliffs.fl && !cliffs.bl {
                    if drive_state.command == Command::Standby {
                        drive_state.command = Command::Advance;
                        motors.lock(|motors| continue_current_heading(drive_state.heading, motors));
                    };
                    return;
                }

                if cliffs.br && cliffs.fr && cliffs.fl && cliffs.bl {
                    drive_state.command = Command::Standby;
                    motors.lock(|motors| stop(motors));
                }

                if drive_state.command == Command::Advance {
                    // Reaching this point means one or more cliffs has been detected
                    drive_state.heading = drive_state.heading.toggle();
                    drive_state.command = Command::PreTurn;
                    drive_state.turn_direction = if cliffs.fr || cliffs.bl {
                        TurnDirection::Right
                    } else {
                        TurnDirection::Left
                    };
                    drive_state.pre_turn_count = 0;
                    drive_state.turn_count = 0;
                    motors.lock(|motors| continue_current_heading(drive_state.heading, motors));
                }
            });

            // hprintln!("drive_state {:?}", drive_state).unwrap();
            match drive_state.command {
                Command::Advance => {}
                Command::PreTurn => {
                    drive_state.pre_turn_count += 1;
                }
                Command::Turn => {
                    drive_state.turn_count += 1;
                }
                Command::Standby => (),
            }

            if drive_state.pre_turn_count == PRE_TURN_COUNT_THRESHOLD {
                drive_state.command = Command::Turn;
                drive_state.pre_turn_count = 0;
                motors.lock(|motors| continue_current_turn(drive_state.turn_direction, motors));
            }

            if drive_state.turn_count == TURN_COUNT_THRESHOLD {
                drive_state.command = Command::Advance;
                drive_state.turn_count = 0;
                motors.lock(|motors| continue_current_heading(drive_state.heading, motors));
            }
        }
    }

    fn continue_current_heading(heading: Heading, motors: &mut MotorsType) {
        match heading {
            Heading::Forward => {
                motors.a.forward();
                motors.b.forward();
            }
            Heading::Reverse => {
                motors.a.reverse();
                motors.b.reverse();
            }
        }
    }

    fn continue_current_turn(turn_direction: TurnDirection, motors: &mut MotorsType) {
        match turn_direction {
            TurnDirection::Left => {
                motors.a.forward();
                motors.b.reverse();
            }
            TurnDirection::Right => {
                motors.a.reverse();
                motors.b.forward();
            }
        }
    }

    fn stop(motors: &mut MotorsType) {
        motors.a.stop();
        motors.b.stop();
    }
}
