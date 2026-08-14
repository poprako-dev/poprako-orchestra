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
pub trait Context {
    /// The actual transaction level provided by this context.
    type Level: Level;
}

/// Guards a step invocation on `Self`, stating that the provided level covers
/// the required one.
///
/// Unlike [`AtLeast`] — a relation between two level types — this is a bound
/// on the stepper itself, which lets `#[drive]` hoist per-operation level
/// requirements into aggregate-trait supertraits. Supertraits are assumed for
/// callers, so a generic usecase only needs the aggregate bound, while the
/// blanket impl below still forces the underlying [`AtLeast`] relation at
/// concrete instantiation.
pub trait LevelGuard<Provided: Level, Required: Level> {}

impl<P, R, S> LevelGuard<P, R> for S
where
    P: Level,
    R: Level,
    P: AtLeast<R>,
{
}
