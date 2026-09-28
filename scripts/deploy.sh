#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

env_file="${DEPLOY_ENV_FILE:-/srv/mindfolio/config/production.env}"
app_file=deploy/compose.app.yaml
edge_file=deploy/compose.edge.yaml
portainer_file=deploy/compose.portainer.yaml

require_env_file() {
  [[ -f "$env_file" ]] || {
    printf '缺少部署环境文件：%s\n' "$env_file" >&2
    exit 1
  }
}

app_compose() {
  require_env_file
  docker compose --env-file "$env_file" --file "$app_file" "$@"
}

edge_compose() {
  require_env_file
  docker compose --env-file "$env_file" --file "$edge_file" "$@"
}

case "${1:-}" in
  check)
    require_env_file
    docker compose --file "$portainer_file" config --quiet
    app_compose config --quiet
    edge_compose config --quiet
    printf '三个 Compose 配置检查通过\n'
    ;;
  network:create)
    network="${EDGE_NETWORK:-mindfolio_edge}"
    if ! docker network inspect "$network" >/dev/null 2>&1; then
      docker network create "$network" >/dev/null
    fi
    printf '共享网络已就绪：%s\n' "$network"
    ;;
  portainer:up)
    docker compose --file "$portainer_file" up --detach --wait
    ;;
  db:up)
    app_compose up --detach --wait postgres
    ;;
  migrate)
    app_compose run --rm --no-deps --entrypoint /usr/local/bin/migrate api
    ;;
  admin:init | admin:reset)
    action="${1#admin:}"
    if [[ "$action" == init ]]; then
      [[ -n "${ADMIN_USERNAME:-}" ]] || {
        printf '请在命令环境中设置 ADMIN_USERNAME\n' >&2
        exit 1
      }
      app_compose run --rm --no-deps --env "ADMIN_USERNAME=$ADMIN_USERNAME" \
        --entrypoint /usr/local/bin/admin api init
    else
      app_compose run --rm --no-deps --entrypoint /usr/local/bin/admin api reset
    fi
    ;;
  app:up)
    app_compose up --detach --wait
    ;;
  edge:up)
    edge_compose up --detach --wait
    ;;
  edge:reload)
    edge_compose exec -T edge caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
    edge_compose exec -T edge caddy reload --config /etc/caddy/Caddyfile --adapter caddyfile
    ;;
  status)
    docker compose --file "$portainer_file" ps
    app_compose ps
    edge_compose ps
    ;;
  *)
    printf '用法：%s {check|network:create|portainer:up|db:up|migrate|admin:init|admin:reset|app:up|edge:up|edge:reload|status}\n' "$0" >&2
    exit 2
    ;;
esac
