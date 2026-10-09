#!/usr/bin/env bash
# Popula os bancos descartáveis do docker-compose.yml ao lado.
#
#   seed.sh up                      sobe os containers e espera ficarem prontos
#   seed.sh pg-tenants <N> [W]      garante N schemas tenant_* no database `tenants`
#                                   (incremental: só cria os que faltam; W workers)
#   seed.sh pg-dbs <N>              N databases `tenantdb_NNN` com 150 tabelas cada
#   seed.sh mongo <coleções> <dbs>  coleções no database `tenants` + dbs `tenantdb_NNN`
#   seed.sh stats                   tamanho em disco e contagens do catálogo
#   seed.sh compat                  sobe o Postgres 14 (porta 55433) com 50 tenants
#   seed.sh down                    remove containers **e volumes**
#
# PG_CONTAINER escolhe o Postgres a popular (padrão: octapus-perf-pg).
set -euo pipefail

cd "$(dirname "$0")"
COMPOSE=(docker compose -p octapus-perf -f docker-compose.yml)
PG_CONTAINER=${PG_CONTAINER:-octapus-perf-pg}
PG=(docker exec -i "$PG_CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 -qAt)
TABLES_PER_TENANT=150

wait_pg() {
  until docker exec "$PG_CONTAINER" pg_isready -U postgres -q; do sleep 1; done
}

ensure_db() {
  local db=$1
  if [[ -z "$("${PG[@]}" -c "SELECT 1 FROM pg_database WHERE datname = '$db'")" ]]; then
    "${PG[@]}" -c "CREATE DATABASE $db"
  fi
}

case "${1:-}" in
  up)
    "${COMPOSE[@]}" up -d
    wait_pg
    until docker exec octapus-perf-mongo mongosh --quiet --eval 'db.runCommand({ ping: 1 }).ok' >/dev/null 2>&1; do
      sleep 1
    done
    echo "pronto: postgres em 55432, mongo em 57017"
    ;;

  pg-tenants)
    total=${2:?informe o total de schemas}
    workers=${3:-8}
    wait_pg
    ensure_db tenants
    "${PG[@]}" -d tenants -f /seed/pg-tenants.sql
    "${PG[@]}" -d tenants -c "CALL perf_seed_shared(300)"

    start=$(date +%s)
    chunk=$(( (total + workers - 1) / workers ))
    pids=()
    for (( w = 0; w < workers; w++ )); do
      first=$(( w * chunk + 1 ))
      last=$(( (w + 1) * chunk < total ? (w + 1) * chunk : total ))
      (( first > last )) && continue
      "${PG[@]}" -d tenants -c "CALL perf_seed_tenants($first, $last, $TABLES_PER_TENANT)" &
      pids+=($!)
    done
    for pid in "${pids[@]}"; do wait "$pid"; done
    # Um -c por comando: VACUUM não roda dentro do bloco implícito de multi-comando
    "${PG[@]}" -d tenants -c "VACUUM ANALYZE pg_class" -c "VACUUM ANALYZE pg_namespace" \
      -c "VACUUM ANALYZE pg_attribute"
    echo "pg-tenants: $total schemas em $(( $(date +%s) - start ))s"
    ;;

  pg-dbs)
    total=${2:?informe o total de databases}
    wait_pg
    ensure_db tenantdb_template
    "${PG[@]}" -d tenantdb_template -f /seed/pg-tenants.sql
    # Template = um único tenant no schema public
    if [[ -z "$("${PG[@]}" -d tenantdb_template -c "SELECT 1 FROM pg_tables WHERE schemaname = 'public' LIMIT 1")" ]]; then
      "${PG[@]}" -d tenantdb_template -c "DO \$\$ BEGIN
        FOR t IN 1..$TABLES_PER_TENANT LOOP
          EXECUTE format('CREATE TABLE public.%I (id bigint PRIMARY KEY, tenant_ref int NOT NULL, name text, payload jsonb, created_at timestamptz NOT NULL DEFAULT now())', perf_table_name(t));
        END LOOP; END \$\$"
    fi
    for (( d = 1; d <= total; d++ )); do
      name=$(printf 'tenantdb_%03d' "$d")
      if [[ -z "$("${PG[@]}" -c "SELECT 1 FROM pg_database WHERE datname = '$name'")" ]]; then
        "${PG[@]}" -c "CREATE DATABASE $name TEMPLATE tenantdb_template"
      fi
    done
    echo "pg-dbs: $total databases"
    ;;

  mongo)
    collections=${2:?informe o total de coleções}
    databases=${3:?informe o total de databases}
    docker exec octapus-perf-mongo mongosh --quiet \
      --eval "const COLLECTIONS=$collections, DATABASES=$databases, PER_DB=20" /seed/mongo.js
    ;;

  stats)
    "${PG[@]}" -d tenants -c "
      SELECT 'schemas', count(*) FROM pg_namespace WHERE nspname LIKE 'tenant_%'
      UNION ALL SELECT 'pg_class', count(*) FROM pg_class
      UNION ALL SELECT 'tabelas', count(*) FROM pg_class WHERE relkind IN ('r','p')" | column -t -s '|'
    "${PG[@]}" -c "SELECT datname, pg_size_pretty(pg_database_size(datname)) FROM pg_database
                   WHERE datname IN ('tenants', 'tenantdb_template')" | column -t -s '|'
    docker system df -v 2>/dev/null | grep -E 'octapus-perf_(pg|mongo)data' || true
    ;;

  compat)
    "${COMPOSE[@]}" --profile compat up -d postgres14
    PG_CONTAINER=octapus-perf-pg14 "$0" pg-tenants 50 2
    ;;

  down)
    "${COMPOSE[@]}" --profile compat down -v
    ;;

  *)
    sed -n '2,15p' "$0"
    exit 1
    ;;
esac
