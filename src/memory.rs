// memory.rs v1.3.38
//! Return unused allocator pages without discarding application state.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn mallopt(parameter: std::ffi::c_int, value: std::ffi::c_int) -> std::ffi::c_int;
    fn malloc_trim(pad: usize) -> std::ffi::c_int;
}

pub(crate) fn configure() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: glibc allocator controls are thread-safe and do not invalidate live allocations.
    unsafe {
        mallopt(-8, 2); // M_ARENA_MAX: avoid a retained heap for each short-lived worker.
    }
}

pub(crate) fn release() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: malloc_trim releases only free pages; live Rust allocations remain valid.
    unsafe { malloc_trim(0); }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reclaim_preserves_live_allocations_across_threads() {
        let keep = vec![73u8; 1024 * 1024];
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    let temporary = (0..4096).map(|_|vec![11u8;1024]).collect::<Vec<_>>();
                    drop(temporary);
                    super::release();
                });
            }
        });
        assert!(keep.iter().all(|byte|*byte == 73));
    }
}
