//! # surf-executor
//!
//! Multi-threaded parallel execution that works across native and browser environments.
//!
//! ## Overview
//!
//! This crate provides parallel execution primitives using [Rayon](https://github.com/rayon-rs/rayon),
//! with support for both native platforms and WebAssembly (via `wasm-bindgen-rayon`).
//!
//! ## Features
//!
//! - [`exec_parallel`] - Execute a closure on each item in a slice in parallel
//! - [`exec_scope`] - Execute multiple closures in parallel (fire-and-forget)
//! - [`exec_scope_with_results`] - Execute multiple closures in parallel and collect results
//!
//! ## WASM Setup
//!
//! When targeting WebAssembly, you must initialize the thread pool before using parallel operations:
//!
//! ```javascript,ignore
//! import init, { initThreadPool } from './pkg/surf_executor.js';
//!
//! await init();
//! await initThreadPool(navigator.hardwareConcurrency);
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use surf_executor::{exec_parallel, exec_scope_with_results};
//!
//! // Parallel map operation
//! let numbers = vec![1, 2, 3, 4, 5];
//! let squared: Vec<i32> = exec_parallel(&numbers, |n| n * n);
//! assert_eq!(squared, vec![1, 4, 9, 16, 25]);
//!
//! // Parallel scope - run multiple closures concurrently and collect results
//! let results: Vec<i32> = exec_scope_with_results(vec![
//!     || 1 + 1,
//!     || 2 + 2,
//!     || 3 + 3,
//! ]);
//! assert_eq!(results, vec![2, 4, 6]);
//! ```

#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;

mod parallel;

pub use parallel::{exec_parallel, exec_scope, exec_scope_with_results};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod native_tests;

#[cfg(target_arch = "wasm32")]
#[cfg(test)]
mod wasm_tests;
