
#![no_std]
#![feature(custom_test_frameworks)]
#![cfg_attr(target_family = "bolos", feature(asm_const))]
#![test_runner(crate::my_runner)]
#![no_main]

use nanos_sdk::exit_app;

use alamgu_example::main_nanos::*;

#[no_mangle]
extern "C" fn sample_main() {
    app_main()
}

fn my_runner(_: &[&i32]) {}

use core::panic::PanicInfo;
#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    exit_app(0);
}


//#![cfg(feature = "speculos")]
const RELOC_SIZE: usize =
    if cfg!(feature = "extra_debug") {
        1024 * 10
    } else {
        1024 * 7
    };

::core::arch::global_asm! {
    ".global _reloc_size",
    ".set _reloc_size, {reloc_size}",
    reloc_size = const RELOC_SIZE,
}
