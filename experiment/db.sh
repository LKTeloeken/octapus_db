#!/usr/bin/env bash
# Os bancos de teste do experimento: baixa o BIRD Mini-Dev (PostgreSQL) e o
# AptMQL-Bench (MongoDB), sobe os containers e carrega os dados.
#
#   ./experiment/db.sh download   baixa os dados para experiment/data/ (retoma de onde parou)
#   ./experiment/db.sh up         sobe PostgreSQL, MongoDB e Redis e carrega o que faltar
#   ./experiment/db.sh status     o que está carregado
#   ./experiment/db.sh down       para os containers (os dados ficam)
#   ./experiment/db.sh reset      para e apaga os dados carregados
#
# DATASETS="student_club financial" limita os bancos do MongoDB a baixar e
# carregar (padrão: os 11 do BIRD dev). O PostgreSQL vem num dump só.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA="$HERE/data"
BIRD_ZIP_URL="https://bird-bench.oss-cn-beijing.aliyuncs.com/minidev.zip"
APTMQL="https://huggingface.co/datasets/MongoDB/AptMQL-Bench"
ALL_DATASETS="california_schools card_games codebase_community debit_card_specializing european_football_2 financial formula_1 student_club superhero thrombosis_prediction toxicology"
DATASETS="${DATASETS:-$ALL_DATASETS}"

compose() { docker compose -f "$HERE/docker-compose.yml" "$@"; }

download() {
  mkdir -p "$DATA/bird" "$DATA/aptmql"

  echo ">> BIRD Mini-Dev (perguntas e dump do PostgreSQL, ~800 MB)"
  curl -fL -C - --retry 5 -o "$DATA/bird/minidev.zip" "$BIRD_ZIP_URL"
  # Só o que o experimento usa: o dump do PostgreSQL e os arquivos de perguntas
  # (o zip também traz o MySQL e os SQLite, mais 2,4 GB)
  unzip -q -n "$DATA/bird/minidev.zip" \
    'minidev/MINIDEV_postgresql/*' 'minidev/MINIDEV/mini_dev_postgresql.json' 'minidev/MINIDEV/dev_tables.json' \
    -d "$DATA/bird"

  echo ">> AptMQL-Bench (perguntas e dumps do MongoDB)"
  local list="$DATA/aptmql/.files"
  curl -fsSL "https://huggingface.co/api/datasets/MongoDB/AptMQL-Bench/tree/main?recursive=true" \
    | tr ',' '\n' | grep -o '"path":"[^"]*"' | cut -d'"' -f4 > "$list.all"
  {
    printf '%s\n' aptmqlbench_data.jsonl LICENSE.md ATTRIBUTION.md README.md
    for dataset in $DATASETS; do grep "^databases/dev/$dataset/.*\." "$list.all" || true; done
  } > "$list"
  rm -f "$list.all"
  # shellcheck disable=SC2016
  xargs -P 4 -I{} sh -c '
    target="$1/$3"
    mkdir -p "$(dirname "$target")"
    curl -fsSL -C - --retry 5 -o "$target" "$2/resolve/main/$(printf %s "$3" | sed "s/ /%20/g")" && echo "   $3"
  ' _ "$DATA/aptmql" "$APTMQL" {} < "$list"
  echo "Pronto: dados em $DATA"
}

psql_value() { compose exec -T postgres psql -U postgres -d bird -tA -c "$1"; }
mongo_value() { compose exec -T mongo mongosh --quiet --eval "$1"; }

load_postgres() {
  local dump="$DATA/bird/minidev/MINIDEV_postgresql/BIRD_dev.sql"
  # O último schema a ser criado: se está completo, a carga terminou
  if [ "$(psql_value "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'toxicology'")" = "4" ]; then
    echo "   PostgreSQL: já carregado"
    return
  fi
  [ -f "$dump" ] || { echo "ERRO: sem o dump do PostgreSQL. Rode: ./experiment/db.sh download" >&2; exit 1; }

  echo ">> PostgreSQL: carregando o dump (1 GB; alguns minutos)"
  # Do zero: uma carga interrompida deixa tabelas pela metade
  for dataset in $ALL_DATASETS; do psql_value "DROP SCHEMA IF EXISTS $dataset CASCADE" >/dev/null 2>&1; done
  psql_value "DROP SCHEMA public CASCADE; CREATE SCHEMA public" >/dev/null 2>&1
  # O dump dá as tabelas a quem o gerou
  psql_value "DO \$\$ BEGIN CREATE ROLE xiaolongli; EXCEPTION WHEN duplicate_object THEN NULL; END \$\$" >/dev/null
  compose exec -T postgres psql -q -U postgres -d bird -v ON_ERROR_STOP=1 \
    -f /import/bird/minidev/MINIDEV_postgresql/BIRD_dev.sql >/dev/null
  compose exec -T postgres psql -q -U postgres -d bird -v ON_ERROR_STOP=1 -f /scripts/schemas.sql
  psql_value "ANALYZE" >/dev/null
}

load_mongo() {
  for dataset in $DATASETS; do
    local dir="$DATA/aptmql/databases/dev/$dataset/dump"
    if [ ! -d "$dir" ]; then
      echo "   MongoDB: sem os dados de $dataset (rode ./experiment/db.sh download)"
      continue
    fi
    if [ "$(mongo_value "db.getSiblingDB('$dataset').getCollectionNames().length")" != "0" ]; then
      continue
    fi
    echo ">> MongoDB: carregando $dataset"
    compose exec -T mongo mongorestore --quiet --drop --db "$dataset" "/import/aptmql/databases/dev/$dataset/dump"
  done
  echo "   MongoDB: carregado"
}

up() {
  compose up -d --wait
  load_postgres
  load_mongo
  status
}

status() {
  echo "PostgreSQL (127.0.0.1:55440, database bird): tabelas por schema"
  psql_value "SELECT format('  %-26s %s', table_schema, tables) FROM (
                SELECT table_schema, count(*) AS tables FROM information_schema.tables
                WHERE table_schema NOT IN ('pg_catalog', 'information_schema') GROUP BY table_schema
              ) AS by_schema ORDER BY table_schema"
  echo "MongoDB (127.0.0.1:57040): coleções por database"
  mongo_value "db.adminCommand('listDatabases').databases.filter(d => !['admin','config','local'].includes(d.name))
               .forEach(d => print('  ' + d.name.padEnd(26) + ' ' + db.getSiblingDB(d.name).getCollectionNames().length))"
  echo "Redis (127.0.0.1:56390): $(compose exec -T redis redis-cli dbsize | tr -d '\r') chave(s)"
}

case "${1:-}" in
  download) download ;;
  up) up ;;
  status) status ;;
  down) compose down ;;
  reset) compose down --volumes ;;
  *) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
