//! Run a binary's real `main` on a thread with a large stack.
//!
//! Windows gives a program's main thread 1 MB of stack, where macOS and
//! Linux give 8 MB. Falcon needs more than 2 MB in debug builds just to parse
//! its command line, and analysis walks syntax trees recursively, so on
//! Windows the CLI crashed with `STATUS_STACK_OVERFLOW` (0xC00000FD). Running
//! on our own thread gives every OS the same, larger stack.

/// Stack for the thread that runs `main`. Only reserved address space; pages
/// are committed as they are used.
pub const STACK_BYTES: usize = 16 * 1024 * 1024;

/// Run `f` on a thread named `main` with [`STACK_BYTES`] of stack and wait
/// for it. A panic in `f` is re-raised here, so the exit status is the same
/// as a panic on the real main thread.
pub fn run<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let spawned = std::thread::Builder::new()
        .name("main".to_string())
        .stack_size(STACK_BYTES)
        .spawn(f);
    match spawned {
        Ok(handle) => {
            if let Err(panic) = handle.join() {
                std::panic::resume_unwind(panic);
            }
        }
        Err(e) => {
            eprintln!("error: could not start Falcon's main thread: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Uses about `depth` KB of stack.
    fn recurse(depth: usize) -> u8 {
        let buf = std::hint::black_box([depth as u8; 1024]);
        if depth == 0 {
            buf[0]
        } else {
            recurse(depth - 1).wrapping_add(buf[1023])
        }
    }

    #[test]
    fn runs_the_closure_with_more_stack_than_a_default_thread() {
        // About 6 MB: more than a default 2 MB spawned thread or a 1 MB
        // Windows main thread has, well under STACK_BYTES.
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        run(move || {
            std::hint::black_box(recurse(6 * 1024));
            flag.store(true, Ordering::SeqCst);
        });
        assert!(done.load(Ordering::SeqCst));
    }

    #[test]
    fn a_panic_in_the_closure_reaches_the_caller() {
        let caught = std::panic::catch_unwind(|| run(|| panic!("boom")));
        assert!(caught.is_err());
    }
}
