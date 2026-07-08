//! Defines the [`Step`] trait — the "how" of a transactional operation.
//!
//! A [`Step`] is an *async executor* that processes a given
//! [`Oper`](crate::oper::Oper) against a mutable context.  It encapsulates
//! the logic of running one atomic unit of work inside a transaction.
//!
//! # Relation to [`Oper`](crate::oper::Oper)
//!
//! [`Step`] is parameterised by an [`Oper`](crate::oper::Oper) type and a
//! context type [`C`](Step).  This keeps the execution logic separate from
//! the input data:
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

use async_trait::async_trait;

use crate::oper::Oper;

/// An async executor that processes an [`Oper`] against a given context.
///
/// # Type parameters
///
/// * `O` — the [`Oper`] type this step can execute.  The step extracts
///   input arguments from the oper and writes its result as
///   [`O::Output`](Oper::Output).
/// * `C` — the context type this step requires (e.g. a database connection,
///   a repository handle, or a domain aggregate).
///
/// # Example
///
/// ```ignore
/// use async_trait::async_trait;
/// use poprako_s_atomicity::{oper::Oper, step::Step};
///
/// pub struct CreateUserStep;
///
/// #[async_trait]
/// impl Step<CreateUser, DbConn> for CreateUserStep {
///     type Error = anyhow::Error;
///
///     async fn step(&mut self, cx: &mut DbConn, oper: &CreateUser) -> Result<UserId, Self::Error> {
///         // ... insert user into database ...
///         Ok(user_id)
///     }
/// }
/// ```
#[async_trait]
pub trait Step<O, C>
where
    O: Oper,
{
    /// Error type that may occur during step execution.
    type Error;

    /// Execute the operation against the given context and return its output.
    ///
    /// * `cx` — the mutable context supplied by the caller (often obtained
    ///   from [`Nucl::coord`](crate::nucl::Nucl::coord)).
    /// * `oper` — the operation containing the input arguments.
    async fn step(&mut self, cx: &mut C, oper: &O) -> Result<O::Output, Self::Error>;
}
