#![deny(warnings)]
#![deny(missing_docs)]

//! # future-parking_lot
//!
//! A simple Future implementation for parking_lot

/// parking_lot::Mutex Future implementation
pub mod mutex;
/// parking_lot::RwLock Future implementation
pub mod rwlock;
