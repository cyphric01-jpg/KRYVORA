//! Cooperative cancellation tokens.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cancellation token. Clone freely; clones share the same flag.
///
/// The token is cooperative: it does not interrupt running code. A
/// long-running job must call [`CancelToken::is_cancelled`] at
/// well-defined checkpoints and return early if it is set. This is the
/// only safe way to cancel arbitrary work in Rust without `unsafe`.
#[derive(Debug, Clone)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// Create a fresh, uncancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Request cancellation. Idempotent. Safe to call from any thread.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Query whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_uncancelled() {
        assert!(!CancelToken::new().is_cancelled());
    }

    #[test]
    fn cancel_is_observable_from_clone() {
        let a = CancelToken::new();
        let b = a.clone();
        a.cancel();
        assert!(b.is_cancelled());
    }

    #[test]
    fn cancel_is_idempotent() {
        let t = CancelToken::new();
        t.cancel();
        t.cancel();
        assert!(t.is_cancelled());
    }
}
