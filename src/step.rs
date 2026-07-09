//! Defines the [`Step`] and [`Run`] traits — the "how" of a transactional (or
//! non-transactional) operation.
//!
//! # [`Step`] — inside a transaction
//!
//! A [`Step`] is an *async executor* that processes a given
//! [`Oper`] against a mutable context.  It encapsulates
//! the logic of running one atomic unit of work inside a transaction.
//!
//! # [`Run`] — without a transaction
//!
//! A [`Run`] is an *async executor* that processes an
//! [`Oper`] directly, without any context.  Use it for
//! side effects that should execute outside the transactional boundary
//! (external API calls, publishing events, OSS operations).
//!
//! # Relation to [`Oper`]
//!
//! Both traits are parameterised by an [`Oper`] type.
//! This keeps the execution logic separate from the input data:
//!
//! - The implementor holds no per-call state (it is typically a ZST or a
//!   thin adapter with injected dependencies).
//! - All per-call state resides in the [`Oper`] value.
//!
//! # Context
//!
//! [`Step`] additionally takes a context type `C` — the environment inside
//! which the step executes (e.g. a database connection, a domain aggregate
//! root).  It is provided by the caller (often obtained through
//! [`Nucl::coord`](crate::nucl::Nucl::coord)) and passed by mutable
//! reference to [`step`](Step::step).

use std::future::Future;

use crate::oper::Oper;

/// An async executor that processes an [`Oper`] against a given context.
///
/// # Type parameters
///
/// * `O` — the [`Oper`] type this step can execute.
/// * `C` — the context type this step requires (e.g. a database connection,
///   a repository handle, or a domain aggregate).
pub trait Step<O, C>
where
    O: Oper,
{
    /// Error type that may occur during step execution.
    type Error;

    /// Execute the operation against the given context and return its output.
    ///
    /// * `cx` — the mutable context supplied by the caller.
    /// * `oper` — the operation containing the input arguments.
    fn step(
        &self,
        cx: &mut C,
        oper: &O,
    ) -> impl Future<Output = Result<O::Output, Self::Error>> + Send;
}

/// A non-transactional executor that processes an [`Oper`] directly.
///
/// Unlike [`Step`], which executes inside a managed context (typically a
/// transaction), [`Run`] bypasses the context entirely — it takes only `&self`
/// and the [`Oper`].  Use this trait for operations that don't need transactional
/// guarantees, such as side effects dispatched after a transaction completes
/// (e.g. sending emails, publishing events, calling external APIs).
///
/// # Relation to [`Step`]
///
/// Both traits share the same separation of data ([`Oper`]) from logic (the
/// executor).  Choose [`Step`] when the operation needs a mutable context;
/// choose [`Run`] when the operation is self-contained.
///
/// # Type parameters
///
/// * `O` — the [`Oper`] type this executor can run.
pub trait Run<O>
where
    O: Oper,
{
    /// Error type that may occur during execution.
    type Error;

    /// Execute the operation and return its output.
    ///
    /// * `oper` — the operation containing the input arguments.
    fn run(&self, oper: &O) -> impl Future<Output = Result<O::Output, Self::Error>> + Send;
}
