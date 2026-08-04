#!/bin/sh

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
base_output=$(mktemp)
overlay_output=$(mktemp)

cleanup() {
  rm -f "$base_output" "$overlay_output"
}

trap cleanup EXIT HUP INT TERM

kubectl kustomize "$script_dir/base" >"$base_output"
kubectl kustomize "$script_dir/overlays/civo-perf" >"$overlay_output"

if grep -q '^kind: Secret$' "$overlay_output"; then
  echo "rendered output must not contain committed secrets" >&2
  exit 1
fi

if grep -q 'image: ghcr.io/karanbalani/retsu:latest' "$overlay_output"; then
  echo "Retsu must use an immutable image tag" >&2
  exit 1
fi

if grep -q -- '--config' "$overlay_output"; then
  echo "Retsu runtime configuration must use environment variables" >&2
  exit 1
fi

grep -q '^  namespace: retsu-perf$' "$overlay_output"
grep -q 'image: ghcr.io/karanbalani/retsu:sha-' "$overlay_output"
grep -q '^      storageClassName: civo-volume$' "$overlay_output"
grep -q '^  RETSU_HTTP__BIND_ADDRESS: 0.0.0.0$' "$overlay_output"
grep -q '^  RETSU_LOGGING__FORMAT: json$' "$overlay_output"
grep -q '^  RETSU_CACHE__DISTRIBUTED__URL: redis://dragonfly:6379$' "$overlay_output"
grep -q '^  RETSU_WORKER__MANAGEMENT__BIND_ADDRESS: 0.0.0.0$' "$overlay_output"
grep -q '^        envFrom:$' "$overlay_output"
grep -q '^        - name: RETSU_DATABASE__URL$' "$overlay_output"

if command -v kubeconform >/dev/null 2>&1; then
  kubeconform -strict -summary "$overlay_output"
fi

echo "Kubernetes base and Civo performance overlay rendered successfully"
