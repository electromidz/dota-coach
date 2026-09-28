//! Make cargo notice new migrations.
//!
//! `sqlx::migrate!("./migrations")` in `db::run_migrations` is a proc macro: it
//! reads the directory at compile time and bakes the list of migrations into the
//! binary. It registers the files it *found*, so a migration added afterwards is
//! tracked by nothing — cargo sees no changed input, may skip recompiling, and
//! the binary keeps a migration list one file short of the repository.
//!
//! Declaring the directory here makes it a build input, so touching anything
//! under `migrations/` re-runs this script, which rebuilds the crate, which
//! re-expands the macro. Cargo walks the path recursively, so a new file counts
//! as a change even though the directory's own mtime is what it notices first.
//!
//! # What this does not fix
//!
//! The failure it is easy to mistake this for:
//!
//! ```text
//! migration 23 was previously applied but is missing in the resolved migrations
//! ```
//!
//! That one means the *running binary* is older than the database — an artifact
//! built before the migration existed, connecting to a database a newer build has
//! already migrated. No build script can help, because the stale binary is not
//! being rebuilt at all. The fix is to rebuild whatever is actually running:
//! `cargo build --release`, or `docker compose build backend`. The database is
//! not damaged in that state and needs no repair.
//!
//! Tests never hit either problem — each test binary is its own compilation unit
//! and re-expands the macro — which is why this is a build script and not a test.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
