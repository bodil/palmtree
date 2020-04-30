/// Prefetch some data.
///
/// This function may do nothing, if there's no platform support.
/// All x86 CPUs should have some support.
///
/// Try not to use this excessively. The CPU is usually better at
/// predicting what to prefetch than you are, so don't use it unless
/// you see significant benchmark improvements.
#[cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature(enable = "sse")
)]
pub(crate) unsafe fn prefetch<A>(data: &A) {
    // TODO think more carefully about the locality values.
    #[cfg(core_intrinsics)]
    std::intrinsics::prefetch_read_data(data, 2);
    #[cfg(all(not(core_intrinsics), target_arch = "x86"))]
    std::arch::x86::_mm_prefetch(data as *const _ as *const i8, std::arch::x86::_MM_HINT_T1);
    #[cfg(all(not(core_intrinsics), target_arch = "x86_64"))]
    std::arch::x86_64::_mm_prefetch(
        data as *const _ as *const i8,
        std::arch::x86_64::_MM_HINT_T1,
    );
}
