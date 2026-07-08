//! Defines the [`Step`] trait — the "how" of a transactional operation.
//!
//! A [`Step`] is an *async executor* that processes a given
//! [`Oper`](crate::oper::Oper) against a mutable context.  It encapsulates
//! the logic of running one atomic unit of work inside a transaction.
//!
//! # Relation to [`Oper`](crate::oper::Oper)
//!
//! [`Step`] is parameterised by an [`Oper`](crate::oper::Oper) type and a
//! context type `C`.  This keeps the execution logic separate from the
//! input data:
//!
//! - The [`Step`] implementor holds no per-call state (it is typically a
//!   ZST or a thin adapter with injected dependencies).
//! - All per-call state resides in the [`Oper`](crate::oper::Oper) value.
//!
//! # Context
//!
//! The context `C` is the environment inside which the step executes — for
//! instance, a database connection or a domain aggregate root.  It is
//! provided by the caller (often obtained through
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
    fn step(&self, cx: &mut C, oper: &O) -> impl Future<Output = Result<O::Output, Self::Error>> + Send;
}
