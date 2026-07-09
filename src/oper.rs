//! Defines the [`Oper`] trait — the "what" of a transactional operation.
//!
//! An [`Oper`] describes *one atomic unit of work* inside a transaction.  It
//! carries the input arguments needed to perform that work and declares the
//! type of value produced as a result.
//!
//! # Relation to [`Step`](crate::step::Step)
//!
//! Splitting the concern into [`Oper`] (data) and
//! [`Step`](crate::step::Step) (execution) keeps the two independent:
//!
//! - The same [`Oper`] can be executed by different
//!   [`Step`](crate::step::Step)s in different contexts.
//! - A [`Step`](crate::step::Step) is a stateless executor; all operation-
//!   specific state lives in the [`Oper`] value.
//!
//! This is analogous to the **Command** pattern, where [`Oper`] is the
//! command object and [`Step`](crate::step::Step) is the receiver /
//! handler.

/// A single atomic operation within a transaction.
///
/// Implementors are plain data structs that hold the input parameters
/// required to perform the operation.  The associated [`Output`](Oper::Output) type
/// declares what value is produced when the operation executes
/// successfully.
///
/// # Example
///
/// ```
/// use poprako_orchestra::oper::Oper;
///
/// pub struct CreateUser {
///     pub name: String,
///     pub email: String,
/// }
///
/// impl Oper for CreateUser {
///     type Output = UserId;
/// }
/// ```
pub trait Oper {
    /// The type of value produced when this operation succeeds.
    type Output;
}
