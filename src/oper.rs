//! Defines the [`Oper`] trait — the "what" of an operation.
//!
//! An [`Oper`] carries the input arguments needed to perform one unit of work
//! and declares the type of value produced as a result. Transaction semantics
//! belong to the [`Step`](crate::step::Step) implementation that executes it.
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

/// A single operation.
///
/// Implementors are plain data structs that hold the input parameters
/// required to perform the operation.  The associated [`Output`](Oper::Output) type
/// declares what value is produced when the operation executes
/// successfully.
///
/// Use the [`Oper`] derive macro for a concise definition that generates the
/// trait implementation while leaving the struct declaration entirely under
/// your control. Enable the crate's `macro` feature to use it.
///
/// # Examples
///
/// **Derive macro (preferred):**
///
/// ```ignore
/// use poprako_orchestra::Oper;
///
/// #[derive(Oper)]
/// #[oper(output = ())]
/// pub struct CreateUser {
///     pub name: String,
///     pub email: String,
/// }
/// ```
///
/// **Manual:**
///
/// ```
/// use poprako_orchestra::Oper;
///
/// pub struct CreateUser {
///     pub name: String,
///     pub email: String,
/// }
///
/// impl Oper for CreateUser {
///     type Output = ();
/// }
/// ```
pub trait Oper {
    /// The type of value produced when this operation succeeds.
    type Output;
}
