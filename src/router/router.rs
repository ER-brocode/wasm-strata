use std::collections::HashMap;
use std::sync::Arc;
use dashmap::DashMap;
use wasmtime::Module;

use crate::error::{StrataError, StrataResult};
use crate::runtime::engine::{ExecutionResult, StrataEngine};

/// Configuration for an individual deployed edge micro-function.
#[derive(Debug, Clone)]
pub struct FunctionConfig {
    pub name: String,
    pub entrypoint: String,
    pub fuel_limit: Option<u64>,
    pub max_memory_pages: Option<u32>,
    pub environment: HashMap<String, String>,
}

impl FunctionConfig {
    pub fn new(name: impl Into<String>, entrypoint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entrypoint: entrypoint.into(),
            fuel_limit: None,
            max_memory_pages: None,
            environment: HashMap::new(),
        }
    }

    pub fn with_fuel_limit(mut self, limit: u64) -> Self {
        self.fuel_limit = Some(limit);
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }
}

/// Extracted URL route parameters.
#[derive(Debug, Default, Clone)]
pub struct RouteParams {
    pub params: HashMap<String, String>,
}

impl RouteParams {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(|s| s.as_str())
    }
}

/// Registered function route binding.
pub struct FunctionRoute {
    pub method: String,
    pub path_pattern: String,
    pub config: FunctionConfig,
    pub module: Arc<Module>,
}

/// Ultra-low latency micro-function router dispatching incoming edge requests.
pub struct MicroFunctionRouter {
    engine: Arc<StrataEngine>,
    // Mapping method -> matchit Router storing FunctionRoute index or route identifier
    routes: DashMap<String, matchit::Router<Arc<FunctionRoute>>>,
}

impl MicroFunctionRouter {
    pub fn new(engine: Arc<StrataEngine>) -> Self {
        Self {
            engine,
            routes: DashMap::new(),
        }
    }

    /// Register a micro-function route for a given HTTP method and URL path pattern.
    pub fn register(
        &self,
        method: &str,
        path_pattern: &str,
        config: FunctionConfig,
        wasm_bytes: &[u8],
    ) -> StrataResult<()> {
        let compiled_module = Arc::new(self.engine.compile_module(wasm_bytes)?);
        let route = Arc::new(FunctionRoute {
            method: method.to_uppercase(),
            path_pattern: path_pattern.to_string(),
            config,
            module: compiled_module,
        });

        let mut method_router = self.routes.entry(method.to_uppercase()).or_insert_with(matchit::Router::new);
        method_router
            .insert(path_pattern.to_string(), route)
            .map_err(|e| StrataError::Internal(format!("Failed to insert route pattern '{path_pattern}': {e}")))?;

        Ok(())
    }

    /// Register an existing pre-compiled module under a route pattern.
    pub fn register_module(
        &self,
        method: &str,
        path_pattern: &str,
        config: FunctionConfig,
        module: Arc<Module>,
    ) -> StrataResult<()> {
        let route = Arc::new(FunctionRoute {
            method: method.to_uppercase(),
            path_pattern: path_pattern.to_string(),
            config,
            module,
        });

        let mut method_router = self.routes.entry(method.to_uppercase()).or_insert_with(matchit::Router::new);
        method_router
            .insert(path_pattern.to_string(), route)
            .map_err(|e| StrataError::Internal(format!("Failed to insert route pattern '{path_pattern}': {e}")))?;

        Ok(())
    }

    /// Resolve and execute an incoming micro-function call directly.
    pub fn dispatch(
        &self,
        method: &str,
        path: &str,
        payload: &[u8],
    ) -> StrataResult<(ExecutionResult, RouteParams)> {
        let method_upper = method.to_uppercase();
        let router_ref = self.routes.get(&method_upper).ok_or_else(|| StrataError::RouteNotFound {
            method: method.to_string(),
            path: path.to_string(),
        })?;

        let match_result = router_ref.at(path).map_err(|_| StrataError::RouteNotFound {
            method: method.to_string(),
            path: path.to_string(),
        })?;

        let mut params = RouteParams::default();
        for (k, v) in match_result.params.iter() {
            params.params.insert(k.to_string(), v.to_string());
        }

        let route = match_result.value;
        let exec_result = self.engine.execute(&route.module, &route.config.entrypoint, payload)?;

        Ok((exec_result, params))
    }
}
