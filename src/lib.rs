//! Envie - Ephemeral Environment Manager for Terraform
//!
//! A CLI tool that makes it easy to manage multiple ephemeral environments in Terraform
//! with layered dependencies and flexible resource sharing.

pub mod cli;
pub mod commands;
pub mod common;

// Re-export commonly used types for easier access
pub use common::*;
