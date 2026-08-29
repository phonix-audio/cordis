//! Denormal (subnormal) float control for real-time audio threads.
//!
//! Decaying IIR/reverb/compressor state runs exponentially toward zero. Once a
//! value drops below the smallest normal f32 (~1.18e-38) it becomes a *denormal*,
//! and on x86 the FPU traps such operations to microcode, running 20-100x slower.
//! The symptom is a CPU SPIKE precisely when the transport is STOPPED (every tail
//! is decaying into the denormal range at once) — audible as xruns "even when not
//! playing" despite the render phase being near-idle.
//!
//! The standard fix is to enable FTZ (flush-to-zero) in MXCSR: any SSE operation
//! whose result would be denormal is flushed straight to zero. This is what breaks
//! the cascade in a decaying feedback loop — the multiply that would produce a
//! denormal yields a clean 0, so the next iteration reads 0 (a normal), not a
//! denormal. It is per-thread and sticky, costs one register write, and is RT-safe
//! (no syscall, no allocation). Call `enable_flush_to_zero()` once on every thread
//! that runs audio DSP: the host audio callback and each render-pool worker.
//!
//! (The companion DAZ / denormals-are-zero bit, which zeros denormal *inputs*, is
//! not exposed as a stable `std::arch` intrinsic; FTZ alone is sufficient here
//! because the offending values are all *produced* inside our own DSP loops.)

/// Enable flush-to-zero on the current thread (x86/x86_64). No-op elsewhere.
///
/// Idempotent and effectively free, so calling it at the top of every audio
/// callback is fine — the audio thread is created by the host outside our control,
/// so we cannot rely on a one-time init hook there.
#[inline]
pub fn enable_flush_to_zero() {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::{_MM_SET_FLUSH_ZERO_MODE, _MM_FLUSH_ZERO_ON};
        // SAFETY: writes only the current thread's MXCSR FTZ bit; SSE2 is
        // guaranteed on x86_64.
        unsafe { _MM_SET_FLUSH_ZERO_MODE(_MM_FLUSH_ZERO_ON); }
    }
    #[cfg(target_arch = "x86")]
    {
        use std::arch::x86::{_MM_SET_FLUSH_ZERO_MODE, _MM_FLUSH_ZERO_ON};
        // SAFETY: as above; guarded to SSE-capable x86 builds.
        unsafe { _MM_SET_FLUSH_ZERO_MODE(_MM_FLUSH_ZERO_ON); }
    }
    // On aarch64 the default FPCR already flushes denormals (FZ) for NEON in most
    // configurations; no portable stable intrinsic to force it, so this is a no-op.
}
