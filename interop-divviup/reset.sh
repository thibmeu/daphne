#!/bin/sh

# Clear the storage proxy
curl -X POST http://localhost:8787/internal/delete_all
curl -X POST http://localhost:8788/internal/delete_all

# Generate HPKE config
cargo run --bin dapf -- test-routes add-hpke-config http://localhost:8787/v09/ --kem-alg x25519_hkdf_sha256
cargo run --bin dapf -- test-routes add-hpke-config http://localhost:8788/v09/ --kem-alg x25519_hkdf_sha256

# Configure task
curl -X POST 'http://localhost:8787/v09/internal/test/add_task' -H 'Content-Type: application/json' -d @leader_task.json
curl -X POST 'http://localhost:8788/v09/internal/test/add_task' -H 'Content-Type: application/json' -d @helper_task.json
