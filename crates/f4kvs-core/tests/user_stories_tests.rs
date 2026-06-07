//! Integration test file to include user story tests from subdirectory
//!
//! This file makes the user story tests discoverable by cargo test.
//! The actual test implementations are in the user_stories/ subdirectory.

#[path = "user_stories/basic_operations.rs"]
mod basic_operations;

#[path = "user_stories/batch_operations.rs"]
mod batch_operations;

#[path = "user_stories/error_handling.rs"]
mod error_handling;

#[path = "user_stories/scan_operations.rs"]
mod scan_operations;

#[path = "user_stories/concurrent_access.rs"]
mod concurrent_access;

#[path = "user_stories/performance.rs"]
mod performance;
