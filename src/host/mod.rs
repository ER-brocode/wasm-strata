pub mod kv;
pub mod net;

use wasmtime::Linker;
use crate::error::StrataResult;
use crate::runtime::engine::HostState;

/// Registers all WASI and custom host capability functions into the Wasmtime Linker.
pub fn register_all_host_functions(linker: &mut Linker<HostState>) -> StrataResult<()> {
    kv::register_kv_host_functions(linker)?;
    net::register_net_host_functions(linker)?;
    Ok(())
}
