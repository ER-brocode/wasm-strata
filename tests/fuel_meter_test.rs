use std::sync::Arc;
use wasm_strata::{
    host::kv::InMemoryKvStore,
    host::net::NetworkAcl,
    runtime::engine::{RuntimeConfig, StrataEngine},
    runtime::meter::FuelBudget,
    StrataError,
};

#[test]
fn test_fuel_exhaustion_infinite_loop_prevention() {
    let mut config = RuntimeConfig::default();
    // Set a modest fuel budget of 5,000 units
    config.fuel_budget = FuelBudget::new(5_000);

    let kv = Arc::new(InMemoryKvStore::new());
    let net = Arc::new(NetworkAcl::new());
    let engine = StrataEngine::new(config, kv, net).expect("Engine initialization");

    // Infinite loop in WebAssembly
    let wat_infinite_loop = r#"
        (module
            (func (export "spin") (result i32)
                (loop $infinite
                    br $infinite
                )
                i32.const 0
            )
        )
    "#;

    let wasm_bytes = wat::parse_str(wat_infinite_loop).expect("Valid WAT");
    let module = engine.compile_module(&wasm_bytes).expect("Compilation succeeds");

    let result = engine.execute(&module, "spin", &[]);
    match result {
        Err(StrataError::FuelExhausted { consumed, limit }) => {
            assert!(consumed >= limit, "Consumed fuel must meet or exceed limit");
        }
        other => panic!("Expected StrataError::FuelExhausted, got: {:?}", other),
    }
}

#[test]
fn test_deterministic_fuel_consumption() {
    let mut config = RuntimeConfig::default();
    config.fuel_budget = FuelBudget::new(100_000);

    let kv = Arc::new(InMemoryKvStore::new());
    let net = Arc::new(NetworkAcl::new());
    let engine = StrataEngine::new(config, kv, net).expect("Engine init");

    // Deterministic arithmetic loop: compute sum from 1 to 100
    let wat_counter = r#"
        (module
            (func (export "sum") (result i32)
                (local $i i32)
                (local $acc i32)
                (local.set $i (i32.const 1))
                (local.set $acc (i32.const 0))
                (block $exit
                    (loop $cont
                        (local.set $acc (i32.add (local.get $acc) (local.get $i)))
                        (local.set $i (i32.add (local.get $i) (i32.const 1)))
                        (br_if $cont (i32.le_s (local.get $i) (i32.const 100)))
                    )
                )
                local.get $acc
            )
        )
    "#;

    let wasm_bytes = wat::parse_str(wat_counter).expect("WAT parsing");
    let module = engine.compile_module(&wasm_bytes).expect("Module compiled");

    let exec1 = engine.execute(&module, "sum", &[]).expect("Run 1");
    let exec2 = engine.execute(&module, "sum", &[]).expect("Run 2");

    assert_eq!(exec1.status_code, 5050);
    assert_eq!(exec2.status_code, 5050);
    assert_eq!(exec1.metrics.fuel_consumed, exec2.metrics.fuel_consumed, "Fuel consumption must be identical and deterministic");
}
