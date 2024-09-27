#!/bin/env bash

set -e

npm start

collection_url="$(COLLECTOR_BEARER_TOKEN="I-am-the-collector" cargo run --bin dapf leader collect --leader-url http://localhost:8787/v09/ --task-id 8TuT5Z5fAuutsX9DZWSqkUw6pzDl96d3tdsDJgWH2VY < query.json)"

npm start
npm start

curl -X POST http://localhost:8787/internal/process

printf "\n\ncurl -X POST -H 'dap-auth-token:I-am-the-collector' -H 'Content-Type:application/dap-collect-req' '${collection_url}'\n"