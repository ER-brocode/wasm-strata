use crate::error::{StrataError, StrataResult};

/// WebAssembly page size is defined as 64 KiB (65,536 bytes).
pub const WASM_PAGE_SIZE: usize = 65_536;

/// Configuration defining memory boundaries for a sandboxed WebAssembly instance.
#[derive(Debug, Clone, Copy)]
pub struct MemoryLimits {
    /// Initial memory in WebAssembly pages (64 KiB each). Default: 1 page.
    pub min_pages: u32,
    /// Maximum allowable memory in WebAssembly pages. Default: 16 pages (1 MiB).
    pub max_pages: u32,
    /// Absolute hard cap in total bytes across all buffers allocated by the guest.
    pub max_byte_cap: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            min_pages: 1,
            max_pages: 16, // 1 MiB max
            max_byte_cap: 16 * WASM_PAGE_SIZE,
        }
    }
}

impl MemoryLimits {
    pub fn new(min_pages: u32, max_pages: u32) -> Self {
        let max_pages = max_pages.max(min_pages);
        Self {
            min_pages,
            max_pages,
            max_byte_cap: (max_pages as usize).saturating_mul(WASM_PAGE_SIZE),
        }
    }

    pub fn with_byte_cap(mut self, max_bytes: usize) -> Self {
        self.max_byte_cap = max_bytes;
        self
    }
}

/// Dynamic bounds checker ensuring guest memory pointers and allocations strictly
/// adhere to capability and sandbox boundaries.
#[derive(Debug, Clone)]
pub struct MemoryBoundsChecker {
    limits: MemoryLimits,
}

impl MemoryBoundsChecker {
    pub fn new(limits: MemoryLimits) -> Self {
        Self { limits }
    }

    pub fn limits(&self) -> &MemoryLimits {
        &self.limits
    }

    /// Verifies that an allocation request of `new_pages` does not exceed max limits.
    pub fn check_page_growth(&self, current_pages: u32, delta_pages: u32) -> StrataResult<u32> {
        let target_pages = current_pages.saturating_add(delta_pages);
        if target_pages > self.limits.max_pages {
            return Err(StrataError::MemoryBoundsExceeded {
                requested_bytes: (target_pages as usize).saturating_mul(WASM_PAGE_SIZE),
                max_bytes: self.limits.max_byte_cap,
                page_limit: self.limits.max_pages,
            });
        }

        let target_bytes = (target_pages as usize).saturating_mul(WASM_PAGE_SIZE);
        if target_bytes > self.limits.max_byte_cap {
            return Err(StrataError::MemoryBoundsExceeded {
                requested_bytes: target_bytes,
                max_bytes: self.limits.max_byte_cap,
                page_limit: self.limits.max_pages,
            });
        }

        Ok(target_pages)
    }

    /// Verifies that an offset + length memory read/write operation is strictly within the
    /// allocated linear memory slice.
    pub fn validate_range(&self, memory_slice_len: usize, offset: usize, length: usize) -> StrataResult<()> {
        let end = offset.checked_add(length).ok_or_else(|| {
            StrataError::MemoryAccessOutOfBounds {
                offset,
                length,
                total_size: memory_slice_len,
            }
        })?;

        if end > memory_slice_len {
            return Err(StrataError::MemoryAccessOutOfBounds {
                offset,
                length,
                total_size: memory_slice_len,
            });
        }

        Ok(())
    }
}

/// Helper guard for safely reading and writing linear memory slices.
pub struct LinearMemoryGuard<'a> {
    memory: &'a mut [u8],
    checker: &'a MemoryBoundsChecker,
}

impl<'a> LinearMemoryGuard<'a> {
    pub fn new(memory: &'a mut [u8], checker: &'a MemoryBoundsChecker) -> Self {
        Self { memory, checker }
    }

    pub fn read_bytes(&self, offset: usize, length: usize) -> StrataResult<&[u8]> {
        self.checker.validate_range(self.memory.len(), offset, length)?;
        Ok(&self.memory[offset..offset + length])
    }

    pub fn write_bytes(&mut self, offset: usize, data: &[u8]) -> StrataResult<()> {
        self.checker.validate_range(self.memory.len(), offset, data.len())?;
        self.memory[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub fn read_null_terminated_string(&self, offset: usize, max_len: usize) -> StrataResult<String> {
        self.checker.validate_range(self.memory.len(), offset, 1)?;
        let available = self.memory.len() - offset;
        let search_len = available.min(max_len);

        let slice = &self.memory[offset..offset + search_len];
        if let Some(pos) = slice.iter().position(|&b| b == 0) {
            String::from_utf8(slice[..pos].to_vec())
                .map_err(|e| StrataError::Internal(format!("Invalid UTF-8 string: {e}")))
        } else {
            Err(StrataError::MemoryAccessOutOfBounds {
                offset,
                length: search_len,
                total_size: self.memory.len(),
            })
        }
    }

    pub fn current_size(&self) -> usize {
        self.memory.len()
    }
}
