#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

scenario="${1:-smoke}"
case "${scenario}" in
    smoke | enqueue | consume | mixed | saturation | showcase) ;;
    *)
        echo "Unknown load scenario '${scenario}'." >&2
        echo "Choose one of: smoke, enqueue, consume, mixed, saturation, or showcase." >&2
        exit 2
        ;;
esac

compose=(docker compose --file infra/local/compose.yaml)
./scripts/local/ready.sh

run_id="${RUN_ID:-local-${RANDOM}-${RANDOM}-$$}"
queue_prefix="${QUEUE_PREFIX:-retsu-k6}"
if [[ "${scenario}" == "showcase" ]]; then
    queue_prefix="${QUEUE_PREFIX:-retsu-showcase}"
fi

grafana_address="$(
    "${compose[@]}" --profile observability port grafana 3000 \
        | tail -n 1
)"
if [[ "${scenario}" == "showcase" ]]; then
    dashboard_path="retsu-showcase/retsu-showcase"
    echo "Showcase run time: ${SHOWCASE_DURATION_MINUTES:-5} minutes"
else
    dashboard_path="retsu-performance/retsu-performance"
fi
dashboard_url="http://${grafana_address}/d/${dashboard_path}"

echo "Running ${scenario} load scenario with RUN_ID=${run_id}"
echo "Watch Retsu while it runs: ${dashboard_url}"
RUN_ID="${run_id}" \
    QUEUE_PREFIX="${queue_prefix}" \
    "${compose[@]}" --profile load run --rm --no-deps \
    --env RUN_ID="${run_id}" \
    --env QUEUE_PREFIX="${queue_prefix}" \
    k6 run "/scripts/${scenario}.js"

echo "Load run complete. Review it in Grafana: ${dashboard_url}"
echo "Use the dashboard time picker to focus on the run that just completed."
