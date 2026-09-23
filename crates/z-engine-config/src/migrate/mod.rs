//! v1 -> v2 settings migration. A file without a `schema` key is v1.

mod convert;
mod file;

pub(crate) use convert::convert_v1;
pub(crate) use file::persist;
pub use file::{MigrationOutcome, migrate_file};
