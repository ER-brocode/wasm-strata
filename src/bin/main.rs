use std::sync::Arc;
use clap::Parser;
use wasm_strata::{
    host::kv::InMemoryKvStore,
    host::net::NetworkAcl,
    router::router::{FunctionConfig, MicroFunctionRouter},
    runtime::engine::{RuntimeConfig, StrataEngine},
};

#[derive(Parser, Debug)]
#[command(author = "koushik4475", version, about = "wasm-strata edge micro-runtime & FaaS engine")]
struct Cli {
    #[arg(short, long, default_value = "8080")]
    port: u16,

    #[arg(short, long, default_value = "1000000")]
    fuel_limit: u64,

    #[arg(short, long, default_value = "16")]
    max_memory_pages: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    println!("⚡ Starting wasm-strata edge micro-runtime v{}", env!("CARGO_PKG_VERSION"));
    println!("   Default fuel limit: {}", cli.fuel_limit);
    println!("   Linear memory ceiling: {} pages ({} KiB)", cli.max_memory_pages, cli.max_memory_pages * 64);

    let mut config = RuntimeConfig::default();
    config.fuel_budget.initial_fuel = cli.fuel_limit;
    config.memory_limits.max_pages = cli.max_memory_pages;

    let kv_store = Arc::new(InMemoryKvStore::new());
    let mut net_acl = NetworkAcl::new();
    net_acl = net_acl.allow_host("api.edge.internal").allow_port(443);
    let net_acl = Arc::new(net_acl);

    let engine = Arc::new(StrataEngine::new(config, kv_store, net_acl)?);
    let router = Arc::new(MicroFunctionRouter::new(engine));

    // Register a demo edge function in WAT format
    let sample_wat = r#"
        (module
            (func (export "handle") (result i32)
                i32.const 42
            )
        )
    "#;
    let wasm_bytes = wat::parse_str(sample_wat)?;
    let func_config = FunctionConfig::new("demo-echo", "handle");
    router.register("GET", "/api/v1/echo", func_config, &wasm_bytes)?;

    println!("✔ Registered route GET /api/v1/echo -> [demo-echo:handle]");

    // Execute test invocation
    let (res, _params) = router.dispatch("GET", "/api/v1/echo", &[])?;
    println!("✔ Invocation verified! Status: {}, Duration: {} μs, Fuel consumed: {}",
        res.status_code, res.metrics.duration_micros, res.metrics.fuel_consumed);

    println!("🚀 wasm-strata edge engine initialized successfully.");
    Ok(())
}
