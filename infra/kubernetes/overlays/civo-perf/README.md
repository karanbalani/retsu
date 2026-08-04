# Civo performance deployment

This overlay runs Retsu, PostgreSQL 18, and Dragonfly in the `retsu-perf`
namespace. It creates internal `ClusterIP` services only. It does not expose
Retsu outside the cluster.

PostgreSQL uses a 20 GiB `civo-volume` claim. Dragonfly is an ephemeral,
memory-bounded cache. The namespace and database are intended for disposable
performance tests, not production data.

Non-sensitive Retsu settings are supplied as `RETSU_*` environment variables
from the `retsu-config` ConfigMap. The database URL is supplied separately from
the `retsu-secrets` Secret. No runtime configuration file is mounted.

## Prepare

Set `images[0].newTag` in `kustomization.yaml` to the immutable tag published
for the Retsu Git commit being tested.

Create the namespace and a local secret file:

```sh
kubectl apply -f infra/kubernetes/overlays/civo-perf/namespace.yaml
cp infra/kubernetes/overlays/civo-perf/secrets.env.example \
  infra/kubernetes/overlays/civo-perf/secrets.env
```

Replace every placeholder in `secrets.env`, then create the Kubernetes Secret
without committing the file:

```sh
kubectl create secret generic retsu-secrets \
  --namespace retsu-perf \
  --from-env-file=infra/kubernetes/overlays/civo-perf/secrets.env \
  --dry-run=client \
  --output yaml \
  | kubectl apply -f -
```

If the GHCR repository is private, also create the required image pull Secret
and add it to the `retsu` ServiceAccount before deployment.

## Deploy

Render locally before applying:

```sh
kubectl kustomize infra/kubernetes/overlays/civo-perf
```

Apply the overlay, then wait for the data services. Application readiness stays
false until PostgreSQL is available and the migration Job has completed:

```sh
kubectl apply -k infra/kubernetes/overlays/civo-perf
kubectl rollout status statefulset/postgres --namespace retsu-perf
kubectl rollout status deployment/dragonfly --namespace retsu-perf
```

Wait for the migration to finish before checking the application rollouts:

```sh
kubectl wait \
  --namespace retsu-perf \
  --for=condition=complete \
  --timeout=10m \
  job/retsu-migrate
kubectl rollout status deployment/retsu-api --namespace retsu-perf
kubectl rollout status deployment/retsu-expired-message-cleaner \
  --namespace retsu-perf
kubectl rollout status deployment/retsu-state-metrics-collector \
  --namespace retsu-perf
```

The API exposes `/health/live`, `/health/ready`, and `/metrics` through the
`retsu-api` Service. Each worker exposes the same operational endpoints through
its own management Service on port `24247`.

Kubernetes Jobs cannot be updated after creation. Before deploying a new image
to the same database, remove only the completed migration Job and re-apply the
overlay:

```sh
kubectl delete job retsu-migrate \
  --namespace retsu-perf \
  --ignore-not-found
kubectl apply -k infra/kubernetes/overlays/civo-perf
```

## Reset or tear down

For a clean benchmark reset, delete the dedicated namespace and recreate the
overlay. This removes the PostgreSQL claim when the `civo-volume` reclaim policy
is `Delete`:

```sh
kubectl get storageclass civo-volume
kubectl delete namespace retsu-perf --wait=true
```

Check for a retained persistent volume after namespace deletion before starting
the next run:

```sh
kubectl get persistentvolume
```

Do not use this reset process for a namespace that contains data you need to
keep.
