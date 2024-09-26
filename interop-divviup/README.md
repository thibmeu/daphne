# Testing interrop with divviup-ts

The goal of this package is to send a DAP report from Node.js, and validate its collection.

## Pre-requisite

Start a server as described in [daphne-server](../crates/daphne-server/README.md). Also run the storage initialisation.

Finally, create a task on the leader and helper. This repository offers an example Prio3Sum task. Use the following to add it to daphne-server helper and leader

```bash
curl -X POST 'http://localhost:8787/v09/internal/test/add_task' -H 'Content-Type: application/json' -d @leader_task.json
curl -X POST 'http://localhost:8788/v09/internal/test/add_task' -H 'Content-Type: application/json' -d @helper_task.json
```

You can use [generate-task](../crates/generate-task/) command line to generate a new task.

## Requirements

* Node 22+
* `npm install`

## Run the test


```bash
npm start
```