use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;
use wasmtime::{Caller, Linker};

use crate::error::{StrataError, StrataResult};
use crate::runtime::engine::HostState;

/// Network Access Control List (ACL) specifying permitted outbound network endpoints.
#[derive(Debug, Clone)]
pub struct NetworkAcl {
    allowed_domains: HashSet<String>,
    allowed_ports: HashSet<u16>,
    allow_all: bool,
}

impl Default for NetworkAcl {
    fn default() -> Self {
        Self {
            allowed_domains: HashSet::new(),
            allowed_ports: HashSet::new(),
            allow_all: false,
        }
    }
}

impl NetworkAcl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_all() -> Self {
        Self {
            allowed_domains: HashSet::new(),
            allowed_ports: HashSet::new(),
            allow_all: true,
        }
    }

    pub fn allow_host(mut self, host: impl Into<String>) -> Self {
        self.allowed_domains.insert(host.into().to_lowercase());
        self
    }

    pub fn allow_port(mut self, port: u16) -> Self {
        self.allowed_ports.insert(port);
        self
    }

    pub fn is_allowed(&self, host: &str, port: u16) -> bool {
        if self.allow_all {
            return true;
        }

        let host_lower = host.to_lowercase();
        let domain_match = self.allowed_domains.contains(&host_lower)
            || self.allowed_domains.iter().any(|d| {
                if d.starts_with("*.") {
                    let suffix = &d[1..];
                    host_lower.ends_with(suffix)
                } else {
                    false
                }
            });

        let port_match = self.allowed_ports.is_empty() || self.allowed_ports.contains(&port);

        domain_match && port_match
    }
}

/// Simulated socket handler for sandboxed micro-functions.
#[derive(Debug, Default)]
pub struct SandboxedSocketHandler {
    open_connections: RwLock<HashSet<u32>>,
}

impl SandboxedSocketHandler {
    pub fn new() -> Self {
        Self {
            open_connections: RwLock::new(HashSet::new()),
        }
    }
}

/// Registers Network socket host function bindings under module namespace "strata_net".
pub fn register_net_host_functions(linker: &mut Linker<HostState>) -> StrataResult<()> {
    // strata_net::connect(host_ptr, host_len, port) -> socket_handle (i32 > 0) or -1 (denied/err)
    linker.func_wrap(
        "strata_net",
        "connect",
        |caller: Caller<'_, HostState>, host_ptr: i32, host_len: i32, port: i32| -> i32 {
            if host_ptr < 0 || host_len < 0 || port < 1 || port > 65535 {
                return -1;
            }

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1,
            };

            let data = mem.data(&caller);
            let checker = caller.data().memory_checker.clone();

            if checker.validate_range(data.len(), host_ptr as usize, host_len as usize).is_err() {
                return -1;
            }

            let host = match std::str::from_utf8(&data[host_ptr as usize..(host_ptr + host_len) as usize]) {
                Ok(s) => s,
                Err(_) => return -1,
            };

            let acl = Arc::clone(&caller.data().net_acl);
            if !acl.is_allowed(host, port as u16) {
                return -1; // Capability Denied by ACL
            }

            // Pseudo socket handle descriptor
            100 + (port % 1000)
        },
    )?;

    // strata_net::send(handle, data_ptr, data_len) -> bytes_sent (i32)
    linker.func_wrap(
        "strata_net",
        "send",
        |caller: Caller<'_, HostState>, handle: i32, data_ptr: i32, data_len: i32| -> i32 {
            if handle <= 0 || data_ptr < 0 || data_len < 0 {
                return -1;
            }

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1,
            };

            let data = mem.data(&caller);
            let checker = caller.data().memory_checker.clone();

            if checker.validate_range(data.len(), data_ptr as usize, data_len as usize).is_err() {
                return -1;
            }

            // In simulated sandbox, data is accepted and acknowledged
            data_len
        },
    )?;

    // strata_net::close(handle) -> i32 (0 = ok)
    linker.func_wrap("strata_net", "close", |_caller: Caller<'_, HostState>, handle: i32| -> i32 {
        if handle > 0 {
            0
        } else {
            -1
        }
    })?;

    Ok(())
}
