//! Provides the [`AsyncFunc`] trait, a workaround for expressing
//! higher-ranked async function bounds on stable Rust.
//!
//! # Motivation
//!
//! Rust's type system cannot directly express a bound like:
//!
//! ```ignore
//! F: for<'a> AsyncFnOnce(&'a mut C) -> Result<T, E>
//! ```
//!
//! while also requiring that the returned future implements [`Send`].
//! The [`AsyncFnOnce`] trait does not yet expose an associated future
//! type that can be constrained in a `where` clause.
//!
//! # The workaround
//!
//! [`AsyncFunc`] extends [`AsyncFnOnce`] with the legacy [`FnOnce`]
//! bound, which *does* expose the returned value as an associated type.
//! By requiring `F: AsyncFunc<T, R>` (and naming `Fut` in the bound),
//! callers can apply [`Send`] (or other bounds) to the future:
//!
//! ```ignore
//! for<'c> F: AsyncFnOnce(&'c mut C) -> StdResult<T, E>
//!     + AsyncFunc<&'c mut C, StdResult<T, E>, Fut: Send>
//!     + Send
//! ```
//!
//! This pattern is used internally by [`Nucl::coord`](crate::nucl::Nucl::coord).

/// A marker trait that pairs [`AsyncFnOnce`] with the equivalent
/// [`FnOnce`] bound, exposing the returned future as an associated type.
///
/// # How to use
///
/// Implementations are blanked for any `F` that satisfies both
/// [`AsyncFnOnce`] and [`FnOnce`], so you never need to implement
/// this trait manually:
///
/// ```ignore
/// fn run<F, T, E>(f: F)
/// where
///     F: AsyncFnOnce() -> Result<T, E>
///         + AsyncFunc<(), Result<T, E>, Fut: Send>;
/// ```
///
/// The `Fut: Send` bound on the associated type ensures the future
/// produced by `f` can be sent across threads.
pub trait AsyncFunc<T, R>:
    AsyncFnOnce(T) -> R + FnOnce(T) -> <Self as AsyncFunc<T, R>>::Fut
{
    /// The concrete [`Future`] type returned by the async function.
    type Fut: Future<Output = R>;
}

impl<F, T, Fut, R> AsyncFunc<T, R> for F
where
    F: AsyncFnOnce(T) -> R + FnOnce(T) -> Fut,
    Fut: Future<Output = R>,
{
    type Fut = Fut;
}
