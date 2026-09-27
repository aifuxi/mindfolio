#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ ! -f .env.db ]]; then
  password="$(node -e 'process.stdout.write(require("node:crypto").randomBytes(24).toString("hex"))')"
  umask 077
  printf 'DB_PASSWORD=%s\nDB_PORT=55432\n' "$password" > .env.db
fi

set -a
source .env.db
set +a
export DATABASE_URL="postgres://mindfolio:${DB_PASSWORD}@127.0.0.1:${DB_PORT}/mindfolio_dev"
export AUTH_ORIGIN="${AUTH_ORIGIN:-http://127.0.0.1:5173}"
compose=(docker compose --env-file .env.db -f compose.db.yaml)

case "${1:-}" in
  start)
    "${compose[@]}" up -d --wait
    ;;
  stop)
    "${compose[@]}" stop
    ;;
  status)
    "${compose[@]}" ps
    ;;
  migrate)
    cargo run --locked -p mindfolio-api --bin migrate
    ;;
  dev-api)
    cargo run --locked -p mindfolio-api --bin mindfolio-api
    ;;
  admin-init)
    cargo run --locked -p mindfolio-api --bin admin -- init
    ;;
  admin-reset)
    cargo run --locked -p mindfolio-api --bin admin -- reset
    ;;
  prepare-test)
    existing="$("${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname = 'mindfolio_test'")"
    if [[ "$existing" != 1 ]]; then
      "${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -v ON_ERROR_STOP=1 -c 'CREATE DATABASE mindfolio_test'
    fi
    export DATABASE_URL="postgres://mindfolio:${DB_PASSWORD}@127.0.0.1:${DB_PORT}/mindfolio_test"
    cargo run --locked -p mindfolio-api --bin migrate
    ;;
  test)
    bash "$0" prepare-test
    export DATABASE_URL="postgres://mindfolio:${DB_PASSWORD}@127.0.0.1:${DB_PORT}/mindfolio_test"
    cargo test --workspace --locked
    ;;
  verify-empty)
    "${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -v ON_ERROR_STOP=1 -c 'DROP DATABASE IF EXISTS mindfolio_verify WITH (FORCE)'
    "${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -v ON_ERROR_STOP=1 -c 'DROP DATABASE IF EXISTS mindfolio_verify_bad WITH (FORCE)'
    "${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -v ON_ERROR_STOP=1 -c 'CREATE DATABASE mindfolio_verify'
    trap '"${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS mindfolio_verify WITH (FORCE)"; "${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS mindfolio_verify_bad WITH (FORCE)"' EXIT
    export DATABASE_URL="postgres://mindfolio:${DB_PASSWORD}@127.0.0.1:${DB_PORT}/mindfolio_verify"
    cargo run --locked -p mindfolio-api --bin migrate
    cargo run --locked -p mindfolio-api --bin migrate
    "${compose[@]}" exec -T postgres psql -U mindfolio -d mindfolio_verify -v ON_ERROR_STOP=1 -tAc "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name IN ('admin_account', 'admin_session', 'project')" | grep -qx 3
    "${compose[@]}" exec -T postgres psql -U mindfolio -d mindfolio_verify -v ON_ERROR_STOP=1 -tAc 'SELECT count(*) FROM _sqlx_migrations WHERE success' | grep -qx 2
    "${compose[@]}" exec -T postgres psql -U mindfolio -d postgres -v ON_ERROR_STOP=1 -c 'CREATE DATABASE mindfolio_verify_bad'
    "${compose[@]}" exec -T postgres psql -U mindfolio -d mindfolio_verify_bad -v ON_ERROR_STOP=1 -c 'CREATE TABLE admin_account (id BIGINT)'
    export DATABASE_URL="postgres://mindfolio:${DB_PASSWORD}@127.0.0.1:${DB_PORT}/mindfolio_verify_bad"
    if output="$(cargo run --locked -p mindfolio-api --bin migrate 2>&1)"; then
      echo '预期迁移冲突失败，但命令成功' >&2
      exit 1
    fi
    if [[ "$output" != *'数据库迁移失败：'* ]]; then
      echo "$output" >&2
      exit 1
    fi
    echo '迁移冲突已明确报错'
    ;;
  *)
    echo '用法：db.sh {start|stop|status|migrate|dev-api|admin-init|admin-reset|prepare-test|test|verify-empty}' >&2
    exit 2
    ;;
esac
