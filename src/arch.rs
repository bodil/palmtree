#[cfg_attr(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_feature(enable = "sse")
)]
pub(crate) unsafe fn prefetch<A>(data: &A) {
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
