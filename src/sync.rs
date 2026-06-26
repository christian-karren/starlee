//! Multi-device sync (PRD: E2EE multi-device sync layer).
//!
//! This module hosts the building blocks of the zero-knowledge sync layer. The
//! first landed piece is [`crypto`] — the key hierarchy and per-record
//! encryption that let a vault be stored as opaque blobs a server cannot read.
//! Later phases (manifest, conflict resolution, transport) will join it here.

pub mod crypto;
