//! Defines the [`Nucl`] trait — the transactional nucleus that provides a
//! managed execution context and coordinates application logic inside it.
//!
//! A *nucleus* is the central coordinator of a transaction.  It supplies a
//! mutable [`Context`](Nucl::Context) and exposes [`coord`](Nucl::coord) to
//! run application logic inside that context.
//!
//! # Error discrimination
//!
//! [`NuclError`] separates two failure categories:
//!
//! - [`Backend`](NuclError::Backend) — infrastructure failures from the
//!   backend itself (begin / commit / rollback).
//! - [`Step`](NuclError::Step) — business-level failures from the closure.
//!
//! The caller pattern-matches on the variant to decide whether to retry
//! ([`Backend`](NuclError::Backend)) or propagate the domain error
//! ([`Step`](NuclError::Step)).

use std::future::Future;
use std::ops::AsyncFnOnce;

/// Discriminates between backend-infrastructure failures and step-level
/// business failures.
pub enum NuclError<BE, SE> {
    /// An error from the transactional backend (connection lost, deadlock, etc.).
    Backend(BE),
    /// An error from the step / business logic executed inside the transaction.
    Step(SE),
}

/// A transactional nucleus that provides a managed [`Context`](Nucl::Context) and coordinates
/// the execution of application logic inside it.
pub trait Nucl {
    /// Error type produced by the backend itself (begin / commit / rollback).
    type Error;

    /// Context type provisioned for each [`coord`](Nucl::coord) call.
    type Context;

    /// Run an async computation inside the nucleus's managed context.
    ///
    /// Returns [`NuclError::Backend`] if the infrastructure fails,
    /// [`NuclError::Step`] if the closure returns an error.
    fn coord<F, T, E>(&self, f: F) -> impl Future<Output = Result<T, NuclError<Self::Error, E>>>
    where
        F: for<'cx> AsyncFnOnce(&'cx mut Self::Context) -> Result<T, E> + Send,
        T: Send,
        E: Send;
}
