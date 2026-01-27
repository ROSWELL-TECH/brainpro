//! Agent service daemon - listens on Unix socket, wraps agent.rs
//! Used by brainpro-agent binary.

#![allow(dead_code)]

pub mod server;
pub mod worker;
