#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

scenario="${1:-smoke}"
case "${scenario}" in
    smoke | enqueue | consume | mixed | saturation | production-day) ;;
    *)
        echo "Unknown load scenario '${scenario}'." >&2
        echo "Choose one of: smoke, enqueue, consume, mixed, saturation, or production-day." >&2
        exit 2
        ;;
esac

compose=(docker compose --file infra/local/compose.yaml)
./scripts/local/ready.sh

run_id="${RUN_ID:-local-${RANDOM}-${RANDOM}-$$}"
echo "Running ${scenario} load scenario with RUN_ID=${run_id}"
RUN_ID="${run_id}" \
    "${compose[@]}" --profile load run --rm --no-deps \
    --env RUN_ID="${run_id}" \
    k6 run "/scripts/${scenario}.js"

grafana_address="$(
    "${compose[@]}" --profile observability port grafana 3000 \
        | tail -n 1
)"
echo "Load run complete. Review it in Grafana: http://${grafana_address}/d/retsu-performance/retsu-performance"
echo "Use the dashboard time picker to focus on the run that just completed."
