#!/bin/sh
set -eu

: "${POSTGRES_USER:?POSTGRES_USER is required}"
: "${POSTGRES_DB:?POSTGRES_DB is required}"
: "${POSTGRES_MONITOR_USER:?POSTGRES_MONITOR_USER is required}"
: "${POSTGRES_MONITOR_PASSWORD:?POSTGRES_MONITOR_PASSWORD is required}"

if [ "$POSTGRES_MONITOR_USER" = "$POSTGRES_USER" ]; then
  echo "POSTGRES_MONITOR_USER must be different from POSTGRES_USER" >&2
  exit 1
fi

psql \
  --set=ON_ERROR_STOP=1 \
  --username "$POSTGRES_USER" \
  --dbname "$POSTGRES_DB" \
  --set=monitor_user="$POSTGRES_MONITOR_USER" \
  --set=monitor_password="$POSTGRES_MONITOR_PASSWORD" <<'SQL'
SELECT format(
    'CREATE ROLE %I LOGIN NOSUPERUSER INHERIT NOCREATEDB NOCREATEROLE NOREPLICATION CONNECTION LIMIT 8 PASSWORD %L',
    :'monitor_user',
    :'monitor_password'
)
WHERE NOT EXISTS (
    SELECT FROM pg_roles WHERE rolname = :'monitor_user'
)
\gexec

SELECT format(
    'ALTER ROLE %I PASSWORD %L CONNECTION LIMIT 8',
    :'monitor_user',
    :'monitor_password'
)
\gexec

SELECT format(
    'ALTER ROLE %I SET search_path TO pg_catalog, public',
    :'monitor_user'
)
\gexec

SELECT format('GRANT pg_monitor TO %I', :'monitor_user')
\gexec

SELECT format(
    'GRANT CONNECT ON DATABASE %I TO %I',
    current_database(),
    :'monitor_user'
)
\gexec

CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
SQL
