#![no_main]

use libfuzzer_sys::fuzz_target;
use palmtree::tests::{integration_test, Input};
use typenum::U64;

fuzz_target!(|input: Input<u8, u8>| {
    integration_test::<U64, U64>(input);
});
