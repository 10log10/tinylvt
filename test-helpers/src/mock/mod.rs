//! Mock data module for TinyLVT testing
//!
//! This module provides realistic test data that can be used across:
//! - Development server (dev-server)
//! - API integration tests
//! - Screenshot automation
//! - Any other testing scenarios

mod desk_allocation;
mod dev_dataset;

pub use desk_allocation::DeskAllocationScreenshot;
pub use dev_dataset::DevDataset;

/// Default timezone for mock data
pub const TZ: &str = "America/Los_Angeles";
