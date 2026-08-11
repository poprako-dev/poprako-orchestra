//! Compile-time transaction-level compatibility.
//!
//! Levels are user-defined marker types. The framework only provides the
//! reflexive relationship; stronger-to-weaker relationships are declared by
//! implementing [`AtLeast`] explicitly.

/// Marks a type as a transaction level.
pub trait Level {}

/// States that `Self` provides at least the guarantees required by `Required`.
pub trait AtLeast<R>: Level
where
    R: Level,
{
}

impl<L> AtLeast<L> for L where L: Level {}

/// Associates an execution context with the transaction level it provides.
pub trait Scope {
    /// The actual transaction level provided by this context.
    type Level: Level;
}
