#!/bin/bash

set -e

npm start
sleep 1

collection_url="$(COLLECTOR_BEARER_TOKEN="I-am-the-collector" cargo run --bin dapf leader collect --leader-url http://localhost:8787/v09/ --task-id 8TuT5Z5fAuutsX9DZWSqkUw6pzDl96d3tdsDJgWH2VY < query.json)"

npm start
sleep 1
npm start
sleep 1

echo "processing..."
curl -X POST http://localhost:8787/internal/process
sleep 1
echo "processing..."
curl -X POST http://localhost:8787/internal/process
sleep 1

printf "\n\ncurl -X POST -H 'dap-auth-token:I-am-the-collector' -H 'Content-Type:application/dap-collect-req' '${collection_url}'\n"
