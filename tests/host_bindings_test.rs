use std::sync::Arc;
use wasm_strata::{
    host::kv::InMemoryKvStore,
    host::net::NetworkAcl,
    runtime::engine::{RuntimeConfig, StrataEngine},
};

#[test]
fn test_host_kv_storage_integration() {
    let config = RuntimeConfig::default();
    let kv = Arc::new(InMemoryKvStore::new());
    let net = Arc::new(NetworkAcl::new());

    // Pre-seed KV Store
    kv.set("session_token", b"valid_auth_token_99".to_vec()).unwrap();

    let engine = StrataEngine::new(config, Arc::clone(&kv), net).unwrap();

    // WebAssembly module importing strata_kv:get and strata_kv:set
    let wat = r#"
        (module
            (import "strata_kv" "get" (func $kv_get (param i32 i32 i32 i32 i32) (result i32)))
            (import "strata_kv" "set" (func $kv_set (param i32 i32 i32 i32) (result i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "session_token")
            (data (i32.const 100) "new_key")
            (data (i32.const 120) "new_value_edge")

            (func (export "test_kv") (result i32)
                (local $res i32)
                ;; 1. Get session_token: key_ptr=0, key_len=13, out_val_ptr=200, max_len=64, written_ptr=300
                (local.set $res (call $kv_get (i32.const 0) (i32.const 13) (i32.const 200) (i32.const 64) (i32.const 300)))
                (if (i32.ne (local.get $res) (i32.const 0))
                    (then (return (i32.const -1)))
                )

                ;; 2. Set new_key -> new_value_edge: key_ptr=100, key_len=7, val_ptr=120, val_len=14
                (local.set $res (call $kv_set (i32.const 100) (i32.const 7) (i32.const 120) (i32.const 14)))
                (if (i32.ne (local.get $res) (i32.const 0))
                    (then (return (i32.const -2)))
                )

                i32.const 0
            )
        )
    "#;

    let wasm_bytes = wat::parse_str(wat).unwrap();
    let module = engine.compile_module(&wasm_bytes).unwrap();
    let exec = engine.execute(&module, "test_kv", &[]).unwrap();

    assert_eq!(exec.status_code, 0);

    // Verify key was inserted into host kv store
    let val = kv.get("new_key").unwrap().expect("Value should exist");
    assert_eq!(val, b"new_value_edge");
}

#[test]
fn test_host_network_acl_enforcement() {
    let config = RuntimeConfig::default();
    let kv = Arc::new(InMemoryKvStore::new());
    let mut acl = NetworkAcl::new();
    // Allow only api.stripe.com on port 443
    acl = acl.allow_host("api.stripe.com").allow_port(443);

    let engine = StrataEngine::new(config, kv, Arc::new(acl)).unwrap();

    let wat = r#"
        (module
            (import "strata_net" "connect" (func $net_connect (param i32 i32 i32) (result i32)))
            (memory (export "memory") 1)
            (data (i32.const 0) "api.stripe.com")
            (data (i32.const 50) "malicious-c2.net")

            (func (export "connect_allowed") (result i32)
                ;; Connect to api.stripe.com (len 14) on port 443 -> should succeed (>0)
                (call $net_connect (i32.const 0) (i32.const 14) (i32.const 443))
            )

            (func (export "connect_denied") (result i32)
                ;; Connect to malicious-c2.net (len 16) on port 80 -> should fail (-1)
                (call $net_connect (i32.const 50) (i32.const 16) (i32.const 80))
            )
        )
    "#;

    let wasm_bytes = wat::parse_str(wat).unwrap();
    let module = engine.compile_module(&wasm_bytes).unwrap();

    let res_allowed = engine.execute(&module, "connect_allowed", &[]).unwrap();
    assert!(res_allowed.status_code > 0, "Authorized connection should return valid descriptor");

    let res_denied = engine.execute(&module, "connect_denied", &[]).unwrap();
    assert_eq!(res_denied.status_code, -1, "Forbidden network connection should return -1");
}
