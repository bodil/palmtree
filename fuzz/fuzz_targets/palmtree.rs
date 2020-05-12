#![no_main]

use libfuzzer_sys::fuzz_target;
use palmtree::tests::{integration_test, Input};

fuzz_target!(|input: Input<u8, u8>| {
    integration_test(input);
});
