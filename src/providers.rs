//! Shared provider traits for dependency injection.
//!
//! This module contains common traits used across the codebase to enable
//! testability through dependency injection. By abstracting external
//! dependencies behind traits, modules can be tested in isolation with
//! mock implementations.

/// Trait for providing timestamps.
///
/// This abstraction enables deterministic testing of time-dependent behavior
/// by allowing injection of mock time providers.
///
/// # Example
///
/// ```
/// use abiogenesis::providers::{TimeProvider, SystemTimeProvider};
///
/// // Production code uses SystemTimeProvider
/// let provider = SystemTimeProvider;
/// let timestamp = provider.now();
/// assert!(timestamp > 0);
/// ```
pub trait TimeProvider: Send + Sync {
    /// Returns the current Unix timestamp in seconds.
    fn now(&self) -> u64;
}

/// Default time provider using system time.
///
/// This is the production implementation that returns the actual
/// current Unix timestamp.
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}