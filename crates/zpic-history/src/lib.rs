//! SQLite-backed upload history.
//!
//! The history store is intentionally simple: a single `uploads` table
//! indexed by `created_at`, with filtering by uploader. The CLI writes a
//! row on every successful upload; `zpic history list` reads them back.

pub mod store;

pub use store::{HistoryEntry, HistoryStore, ListFilter};
