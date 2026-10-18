use thiserror::Error;

#[derive(Error, Debug)]
pub enum StrataError {
    #[error("WebAssembly runtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),

    #[error("Fuel budget exhausted: consumed {consumed} units, limit was {limit}")]
    FuelExhausted { consumed: u64, limit: u64 },

    #[error("Memory bounds violation: requested {requested_bytes} bytes, maximum allowed is {max_bytes} bytes (page limit {page_limit})")]
    MemoryBoundsExceeded {
        requested_bytes: usize,
        max_bytes: usize,
        page_limit: u32,
    },

    #[error("Memory access out-of-bounds at offset {offset} with length {length} (total linear memory size: {total_size})")]
    MemoryAccessOutOfBounds {
        offset: usize,
        length: usize,
        total_size: usize,
    },

    #[error("Function route not found: method '{method}', path '{path}'")]
    RouteNotFound { method: String, path: String },

    #[error("Host function capability violation: {0}")]
    CapabilityDenied(String),

    #[error("Key-Value storage error: {0}")]
    KvStoreError(String),

    #[error("Network I/O forbidden: host '{host}' or port {port} is not in ACL whitelist")]
    NetworkAccessDenied { host: String, port: u16 },

    #[error("Module compilation or snapshot deserialization error: {0}")]
    SnapshotError(String),

    #[error("Internal execution error: {0}")]
    Internal(String),
}

pub type StrataResult<T> = Result<T, StrataError>;
