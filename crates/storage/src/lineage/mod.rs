//! Direct lineage persistence and queries share typed evidence and independent authorization.
mod read;
mod write;
pub use read::{LineageDirection, LineageQuery};
pub(crate) use write::{check_head, persist, prepare};
#[cfg(test)]
mod tests;
