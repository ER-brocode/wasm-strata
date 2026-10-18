pub mod engine;
pub mod memory;
pub mod meter;

pub use engine::{ExecutionMetrics, ExecutionResult, RuntimeConfig, StrataEngine};
pub use memory::{LinearMemoryGuard, MemoryBoundsChecker, MemoryLimits};
pub use meter::{FuelBudget, FuelMeter};
