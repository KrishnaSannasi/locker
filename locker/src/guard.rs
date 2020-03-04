/// Represents an unmapped guard
pub enum Pure {}

/// Represents an mapped guard
pub enum Mapped {}

/// The error return type of `try_map` and `try_split_map`
///
/// Contains the error and the old guard in that order
pub struct TryMapError<E, G>(pub E, pub G);
