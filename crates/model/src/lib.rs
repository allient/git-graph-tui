//! Domain types for tui-git-graph: commit ids, commits, refs (branches, tags,
//! remotes, stashes) and signatures.
//!
//! This crate is pure data. It has no I/O and depends on no other workspace
//! crate, so every other crate can share these types.

mod commit;
mod oid;
mod reference;
mod status;

pub use commit::{Commit, Signature};
pub use oid::{Oid, OidError};
pub use reference::{Ref, RefKind};
pub use status::{HeadState, WorkingTreeStatus};
