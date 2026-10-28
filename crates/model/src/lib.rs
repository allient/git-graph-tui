//! Domain types for tui-git-graph: commit ids, commits, refs (branches, tags,
//! remotes, stashes) and signatures.
//!
//! This crate is pure data. It has no I/O and depends on no other workspace
//! crate, so every other crate can share these types.

mod commit;
mod oid;
mod reference;
mod stash;
mod status;

pub use commit::{Commit, Signature};
pub use oid::{Oid, OidError};
pub use reference::{Ref, RefKind, short_name, split_remote};
pub use stash::Stash;
pub use status::{HeadState, WorkingTreeStatus};
