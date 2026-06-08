//! LCM core library.
//!
//! LCM ("Linux Cert Manager") integrates ready-made certificates into a Linux
//! system. This crate holds the unprivileged domain logic shared by every
//! front end (the `lcm` CLI today, the GTK GUI later):
//!
//! - [`osrelease`] / [`distro`] — detect the distribution family.
//! - [`backend`] — per-distro trust-store backends (where anchors live, how to
//!   apply them). Debian family only for the v1 MVP.
//! - [`cert`] — parse and inspect X.509 certificates (PEM or DER).
//! - [`plan`] — the declarative set of *privileged* operations a front end
//!   hands to the root helper.
//! - [`exec`] — execution of a [`plan::Plan`] (run by the root helper) plus
//!   read-only auditing of what is installed.
//!
//! The privilege boundary lives in [`exec`]: it re-derives the backend itself,
//! re-validates every certificate, and only ever writes to backend-owned
//! directories — it never trusts a path supplied by a front end.

pub mod backend;
pub mod bundle;
pub mod cert;
pub mod distro;
pub mod error;
pub mod exec;
pub mod identity;
pub mod nss;
pub mod osrelease;
pub mod pkcs12;
pub mod plan;
pub mod service;
pub mod skb;
pub mod trust;
pub mod util;

pub use error::{Error, Result};
