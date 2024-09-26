# Testing interrop with divviup-ts

The goal of this package is to send a DAP report from Node.js, and validate its collection.

## Pre-requisite

Start a server as described in [daphne-server](../crates/daphne-server/README.md). Also run the storage initialisation.

Finally, create a task on the leader and helper. This repository offers an example Prio3Sum task. Use the following to add it to daphne-server helper and leader

```bash
# Clear the storage proxy
curl -X POST http://localhost:8787/internal/delete_all
curl -X POST http://localhost:8788/internal/delete_all

# Generate HPKE config
cargo run --bin dapf -- test-routes add-hpke-config http://localhost:8787/v09/ --kem-alg x25519_hkdf_sha256
cargo run --bin dapf -- test-routes add-hpke-config http://localhost:8788/v09/ --kem-alg x25519_hkdf_sha256

# Configure task
curl -X POST 'http://localhost:8787/v09/internal/test/add_task' -H 'Content-Type: application/json' -d @leader_task.json
curl -X POST 'http://localhost:8788/v09/internal/test/add_task' -H 'Content-Type: application/json' -d @helper_task.json
```

You can use [generate-task](../crates/generate-task/) command line to generate a new task.

## Requirements

* Node 22+
* `npm install`

## Run the test

```bash
# Collector initializes collection job
cat query.json | COLLECTOR_BEARER_TOKEN="I-am-the-collector" cargo run --bin dapf leader collect --leader-url http://localhost:8787/v09/ --task-id 8TuT5Z5fAuutsX9DZWSqkUw6pzDl96d3tdsDJgWH2VY

# Client uploads a report.
npm start

# Leader runs aggregation job and completes the collection job.
curl -X POST http://localhost:8787/internal/process

# Collector polls collection job.
XXX
```
