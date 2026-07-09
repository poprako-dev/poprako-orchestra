//! # poprako-orchestra — Transaction Abstraction Framework
//!
//! Provides a layered set of traits for composing and executing transactional
//! operations against an abstract backend (a "nucleus"). The crate decouples
//! three concerns:
//!
//! - **What** to do: described by the [`oper::Oper`] trait, which carries the
//!   operation's input data and declares its [`Output`](oper::Oper::Output)
//!   type.
//! - **How** to do it: described by the [`step::Step`] trait, which receives
//!   an [`Oper`](oper::Oper) and a mutable context, executes the operation,
//!   and returns the output.
//! - **Where** to run it: described by the [`nucl::Nucl`] trait, which
//!   provides a managed context scope where the application can execute
//!   arbitrary async logic with proper error discrimination (backend errors
//!   vs. step-execution errors).
//!
//! ## Layering
//!
//! [`Oper`](oper::Oper) and [`Step`](step::Step) together form the
//! *semantic layer* — they model what your application does inside a
//! transaction.  [`Nucl`](nucl::Nucl) is the *back-end layer* — it models
//! the transactional engine (e.g. a database connection pool, a saga
//! coordinator) that provides the context and handles commit / rollback.

pub mod nucl;
pub mod oper;
pub mod step;

pub use nucl::Nucl;
pub use oper::Oper;
pub use step::{Run, Step};
