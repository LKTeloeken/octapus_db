#!/usr/bin/env bash
# Postgres com TLS para o e2e de src-tauri/src/adapters/postgres/tls_e2e.rs.
#
#   scripts/tls-e2e.sh up     # gera CA + certificado e sobe o container (porta 55433)
#   cd src-tauri && OCTAPUS_TLS_CA=/tmp/octapus-tls-e2e/ca.crt \
#     cargo test --lib postgres::tls_e2e -- --ignored
#   scripts/tls-e2e.sh down   # apaga o container e os certificados
#
# O certificado do servidor é emitido para `db.internal`, não para
# `localhost`: assim o `verify-full` falha pelo nome e o `verify-ca` passa.
set -euo pipefail

DIR=/tmp/octapus-tls-e2e
NAME=octapus-tls-e2e
PORT=55433

case "${1:-}" in
up)
  rm -rf "$DIR" && mkdir -p "$DIR" && cd "$DIR"
  openssl req -x509 -newkey rsa:2048 -nodes -days 2 -subj "/CN=Octapus e2e CA" \
    -addext "basicConstraints=critical,CA:TRUE" -addext "keyUsage=critical,keyCertSign" \
    -keyout ca.key -out ca.crt 2>/dev/null
  openssl req -newkey rsa:2048 -nodes -subj "/CN=db.internal" \
    -keyout server.key -out server.csr 2>/dev/null
  printf 'subjectAltName=DNS:db.internal\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' > server.ext
  openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -days 2 \
    -extfile server.ext -out server.crt 2>/dev/null

  docker rm -f "$NAME" >/dev/null 2>&1 || true
  # A chave precisa ser do usuário postgres e 0600: copia para dentro antes de subir
  docker run -d --name "$NAME" -p "127.0.0.1:$PORT:5432" -e POSTGRES_PASSWORD=tls \
    -v "$DIR:/certs:ro" postgres:16-alpine sh -c '
      cp /certs/server.crt /certs/server.key /var/lib/postgresql/ &&
      chown postgres /var/lib/postgresql/server.* && chmod 600 /var/lib/postgresql/server.key &&
      exec docker-entrypoint.sh postgres -c ssl=on \
        -c ssl_cert_file=/var/lib/postgresql/server.crt \
        -c ssl_key_file=/var/lib/postgresql/server.key' >/dev/null
  until docker exec "$NAME" pg_isready -U postgres >/dev/null 2>&1; do sleep 0.5; done
  echo "Postgres com TLS em localhost:$PORT (CA em $DIR/ca.crt)"
  ;;
down)
  docker rm -f "$NAME" >/dev/null 2>&1 || true
  rm -rf "$DIR"
  ;;
*)
  echo "uso: $0 up|down" >&2
  exit 1
  ;;
esac
