#!/usr/bin/env bash
# Roda as medições do backend (src-tauri/src/perf) contra a fixture já populada.
#
#   run.sh <tier> pg      Postgres: rede local e remota simulada (80 ms / 100 Mbps)
#   run.sh <tier> mongo   Mongo: idem
#
# Binário em release: é o que o app distribui. O teste de lock só roda na rede
# local (ele mede bloqueio, não latência).
set -euo pipefail

tier=${1:?informe o tier (ex.: S, M, L)}
suite=${2:?informe a suíte: pg ou mongo}
cd "$(dirname "$0")/../../src-tauri"

export OCTAPUS_PERF_TIER=$tier
run() {
  cargo test --release --lib "$1" -- --ignored --nocapture --test-threads=1 "${@:2}" 2>&1 \
    | grep -E '^test |test result|panicked' || true
}

case "$suite" in
  pg)
    OCTAPUS_PERF_NET=local run perf::postgres_baseline
    OCTAPUS_PERF_NET=remote run perf::postgres_baseline --skip perf_pg_lock_wait
    ;;
  mongo)
    OCTAPUS_PERF_NET=local run perf::mongo_baseline
    OCTAPUS_PERF_NET=remote run perf::mongo_baseline
    ;;
  *)
    echo "suíte desconhecida: $suite" >&2
    exit 1
    ;;
esac
