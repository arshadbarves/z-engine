//! Per-session settings: trust-aware loading, instruction docs, the model
//! client, the permission policy, and model roles.

mod client;
mod effective;
mod instructions;
pub(crate) mod models;
mod policy;

pub(crate) use client::session_client;
pub(crate) use effective::{SessionSettings, load_session_settings};
pub(crate) use instructions::nested_docs;
pub(crate) use policy::build_policy;
