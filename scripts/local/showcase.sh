#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

duration_minutes="${1:-5}"
if [[ ! "${duration_minutes}" =~ ^[0-9]+$ ]] ||
    ((duration_minutes < 5 || duration_minutes > 20)); then
    echo "Showcase duration must be a whole number from 5 through 20 minutes." >&2
    echo "Usage: just local-showcase [5-20]" >&2
    exit 2
fi

SHOWCASE_DURATION_MINUTES="${duration_minutes}" \
    QUEUE_PREFIX="${QUEUE_PREFIX:-retsu-showcase}" \
    exec ./scripts/local/load.sh showcase
