#![cfg_attr(target_os = "nanos", no_std)]
#![cfg_attr(target_os = "nanos", no_main)]

#[cfg(not(target_os = "nanos"))]
fn main() {}

use rust_app::main_nanos::*;

nanos_sdk::set_panic!(nanos_sdk::exiting_panic);

#[no_mangle]
extern "C" fn sample_main() {
    app_main()
}
