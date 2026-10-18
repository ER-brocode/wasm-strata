use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use wasmtime::{Caller, Linker};

use crate::error::{StrataError, StrataResult};
use crate::runtime::engine::HostState;

/// Trait abstracting Key-Value persistence for edge micro-functions.
pub trait KvStore: Send + Sync {
    fn get(&self, key: &str) -> StrataResult<Option<Vec<u8>>>;
    fn set(&self, key: &str, value: Vec<u8>) -> StrataResult<()>;
    fn delete(&self, key: &str) -> StrataResult<bool>;
    fn contains(&self, key: &str) -> StrataResult<bool>;
}

/// In-memory concurrent Key-Value store implementation.
#[derive(Debug, Default)]
pub struct InMemoryKvStore {
    table: RwLock<HashMap<String, Vec<u8>>>,
}

impl InMemoryKvStore {
    pub fn new() -> Self {
        Self {
            table: RwLock::new(HashMap::new()),
        }
    }
}

impl KvStore for InMemoryKvStore {
    fn get(&self, key: &str) -> StrataResult<Option<Vec<u8>>> {
        let lock = self.table.read();
        Ok(lock.get(key).cloned())
    }

    fn set(&self, key: &str, value: Vec<u8>) -> StrataResult<()> {
        let mut lock = self.table.write();
        lock.insert(key.to_string(), value);
        Ok(())
    }

    fn delete(&self, key: &str) -> StrataResult<bool> {
        let mut lock = self.table.write();
        Ok(lock.remove(key).is_some())
    }

    fn contains(&self, key: &str) -> StrataResult<bool> {
        let lock = self.table.read();
        Ok(lock.contains_key(key))
    }
}

/// Registers Key-Value host function bindings under module namespace "strata_kv".
pub fn register_kv_host_functions(linker: &mut Linker<HostState>) -> StrataResult<()> {
    // strata_kv::get(key_ptr, key_len, out_val_ptr, max_val_len, written_len_ptr) -> i32 (0 = ok, 1 = not found, -1 = error)
    linker.func_wrap(
        "strata_kv",
        "get",
        |mut caller: Caller<'_, HostState>,
         key_ptr: i32,
         key_len: i32,
         out_val_ptr: i32,
         max_val_len: i32,
         written_len_ptr: i32|
         -> i32 {
            if key_ptr < 0 || key_len < 0 || out_val_ptr < 0 || max_val_len < 0 || written_len_ptr < 0 {
                return -1;
            }

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1,
            };

            let data = mem.data(&caller);
            let checker = caller.data().memory_checker.clone();

            if checker.validate_range(data.len(), key_ptr as usize, key_len as usize).is_err() {
                return -1;
            }

            let key_str = match std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize]) {
                Ok(s) => s.to_string(),
                Err(_) => return -1,
            };

            let kv = Arc::clone(&caller.data().kv_store);
            match kv.get(&key_str) {
                Ok(Some(val)) => {
                    let to_write = val.len().min(max_val_len as usize);
                    let mem_slice = mem.data_mut(&mut caller);

                    if checker.validate_range(mem_slice.len(), out_val_ptr as usize, to_write).is_err() {
                        return -1;
                    }
                    if checker.validate_range(mem_slice.len(), written_len_ptr as usize, 4).is_err() {
                        return -1;
                    }

                    mem_slice[out_val_ptr as usize..out_val_ptr as usize + to_write].copy_from_slice(&val[..to_write]);
                    let len_bytes = (to_write as u32).to_le_bytes();
                    mem_slice[written_len_ptr as usize..written_len_ptr as usize + 4].copy_from_slice(&len_bytes);
                    0
                }
                Ok(None) => 1, // Not found
                Err(_) => -1,
            }
        },
    )?;

    // strata_kv::set(key_ptr, key_len, val_ptr, val_len) -> i32 (0 = ok, -1 = error)
    linker.func_wrap(
        "strata_kv",
        "set",
        |mut caller: Caller<'_, HostState>,
         key_ptr: i32,
         key_len: i32,
         val_ptr: i32,
         val_len: i32|
         -> i32 {
            if key_ptr < 0 || key_len < 0 || val_ptr < 0 || val_len < 0 {
                return -1;
            }

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1,
            };

            let data = mem.data(&caller);
            let checker = caller.data().memory_checker.clone();

            if checker.validate_range(data.len(), key_ptr as usize, key_len as usize).is_err() {
                return -1;
            }
            if checker.validate_range(data.len(), val_ptr as usize, val_len as usize).is_err() {
                return -1;
            }

            let key_str = match std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize]) {
                Ok(s) => s.to_string(),
                Err(_) => return -1,
            };
            let value = data[val_ptr as usize..(val_ptr + val_len) as usize].to_vec();

            let kv = Arc::clone(&caller.data().kv_store);
            match kv.set(&key_str, value) {
                Ok(_) => 0,
                Err(_) => -1,
            }
        },
    )?;

    // strata_kv::delete(key_ptr, key_len) -> i32 (1 = deleted, 0 = not found, -1 = error)
    linker.func_wrap(
        "strata_kv",
        "delete",
        |caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32| -> i32 {
            if key_ptr < 0 || key_len < 0 {
                return -1;
            }

            let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                Some(m) => m,
                None => return -1,
            };

            let data = mem.data(&caller);
            let checker = caller.data().memory_checker.clone();

            if checker.validate_range(data.len(), key_ptr as usize, key_len as usize).is_err() {
                return -1;
            }

            let key_str = match std::str::from_utf8(&data[key_ptr as usize..(key_ptr + key_len) as usize]) {
                Ok(s) => s.to_string(),
                Err(_) => return -1,
            };

            let kv = Arc::clone(&caller.data().kv_store);
            match kv.delete(&key_str) {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => -1,
            }
        },
    )?;

    Ok(())
}
