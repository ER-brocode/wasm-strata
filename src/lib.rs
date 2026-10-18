//! # wasm-strata
//!
//! Sub-50us WebAssembly Sandboxed Micro-Runtime & Edge FaaS Engine.
//!
//! Features:
//! - Deterministic instruction & fuel metering to prevent infinite loops and denial-of-service.
//! - Strict linear memory bounds checking with configurable max allocation limits.
//! - Instantaneous micro-function routing with pre-compiled module pooling and snapshot optimization.
//! - Fine-grained capability-based WASI host bindings for Key-Value storage and sandboxed network I/O.

pub mod error;
pub mod host;
pub mod router;
pub mod runtime;

pub use error::{StrataError, StrataResult};
pub use host::kv::{InMemoryKvStore, KvStore};
pub use host::net::{NetworkAcl, SandboxedSocketHandler};
pub use router::router::{FunctionConfig, FunctionRoute, MicroFunctionRouter, RouteParams};
pub use router::snapshot::{ModuleCache, SnapshotOptimizer};
pub use runtime::engine::{ExecutionMetrics, ExecutionResult, RuntimeConfig, StrataEngine};
pub use runtime::memory::{LinearMemoryGuard, MemoryBoundsChecker, MemoryLimits};
pub use runtime::meter::{FuelBudget, FuelMeter};
