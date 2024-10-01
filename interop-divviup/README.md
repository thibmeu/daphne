# Testing interrop with divviup-ts

The goal of this package is to send a DAP report from Node.js, and validate its collection.

## Pre-requisite

Start a server as described in [daphne-server](../crates/daphne-server/README.md). Also run the storage initialisation.

Finally, create a task on the leader and helper. This repository offers an example Prio3Sum task. Use the following to add it to daphne-server helper and leader

```bash
./reset.sh
```

You can use [generate-task](../crates/generate-task/) command line to generate a new task.

Copy the collector configuration from [leader/helper configuration](../crates/daphne-server/examples/configuration-leader.toml) into [hpke_collector_config.json](./hpke_collector_config.json).

## Requirements

* Node 22+
* `npm install`

## Run the test

These commands, except the last one, can be run with `./test.sh`

```bash
./test.sh
# Client uploads a report, which registers the ask somehow
npm start

# Collector initializes collection job
cat query.json | COLLECTOR_BEARER_TOKEN="I-am-the-collector" cargo run --bin dapf leader collect --leader-url http://localhost:8787/v09/ --task-id 8TuT5Z5fAuutsX9DZWSqkUw6pzDl96d3tdsDJgWH2VY

# Client uploads a report. This is the only step we should have to do
npm start
# And a second, which is needed I don't know why
npm start

# Leader runs aggregation job and completes the collection job.
curl -X POST http://localhost:8787/internal/process

# Collector polls collection job.
# Collector init URL looks like http://localhost:8787/v09/collect/task/8TuT5Z5fAuutsX9DZWSqkUw6pzDl96d3tdsDJgWH2VY/req/1qVftsSS8IOH1xh9hvjdRQ
curl -X POST -H 'dap-auth-token:I-am-the-collector' -H 'Content-Type:application/dap-collect-req' <url-from-collector-init>
```
