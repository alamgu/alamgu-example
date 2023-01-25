#![cfg_attr(target_family = "bolos", no_std)]
#![cfg_attr(target_family = "bolos", no_main)]
#![cfg_attr(target_family = "bolos", feature(asm_const))]
#![cfg_attr(target_family = "bolos", feature(cfg_version))]

#[cfg(not(target_family = "bolos"))]
fn main() {}

use alamgu_example::main_nanos::*;

nanos_sdk::set_panic!(nanos_sdk::exiting_panic);

#[no_mangle]
extern "C" fn sample_main() {
    app_main()
}

#[cfg_attr(not(version("1.64")), allow(unused))]
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
