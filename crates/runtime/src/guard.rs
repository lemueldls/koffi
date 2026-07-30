pub fn guarded<R>(f: impl FnOnce() -> R + std::panic::UnwindSafe) -> R {
    match std::panic::catch_unwind(f) {
        Ok(v) => v,
        Err(_) => std::process::abort(),
    }
}
