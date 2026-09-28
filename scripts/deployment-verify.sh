#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

test_id="mindfolio-deploy-verify-$$"
app_project="$test_id-app"
edge_project="$test_id-edge"
portainer_project="$test_id-portainer"
test_dir="$(mktemp -d)"
export API_IMAGE="${IMAGE_API_TAG:-mindfolio-api:local}"
export ADMIN_IMAGE="${IMAGE_ADMIN_TAG:-mindfolio-admin:local}"
export EDGE_IMAGE="${IMAGE_EDGE_TAG:-mindfolio-edge:local}"
export EDGE_NETWORK="$test_id-network"
export PORTAINER_PROXY_NETWORK="$test_id-portainer-proxy"
export PORTAINER_DATA_VOLUME="$test_id-portainer-data"
export PORTAINER_HTTPS_BIND=127.0.0.1:0
export POSTGRES_DATA_VOLUME="$test_id-postgres"
export EDGE_DATA_VOLUME="$test_id-edge-data"
export EDGE_CONFIG_VOLUME="$test_id-edge-config"
export EDGE_CADDY_DIR="$test_dir/caddy"
export EDGE_HTTP_PORT=0
export EDGE_HTTPS_PORT=0
export EDGE_HTTPS_UDP_PORT=0
export ADMIN_DOMAIN=http://localhost
export PORTAINER_DOMAIN=http://portainer.localhost
export PORTAINER_ORIGIN=http://portainer.localhost
export AUTH_ORIGIN=http://localhost
export ALIYUN_ACCESS_KEY_ID=verify-only
export ALIYUN_ACCESS_KEY_SECRET=verify-only
export POSTGRES_PASSWORD
POSTGRES_PASSWORD="$(openssl rand -hex 24)"
export DATABASE_URL="postgres://mindfolio:$POSTGRES_PASSWORD@postgres:5432/mindfolio"

mkdir -p "$EDGE_CADDY_DIR"
cp deploy/edge.Caddyfile "$EDGE_CADDY_DIR/Caddyfile"
cp -R deploy/sites "$EDGE_CADDY_DIR/sites"

cleanup() {
  docker compose --project-name "$portainer_project" --file deploy/compose.portainer.yaml down --volumes --remove-orphans >/dev/null 2>&1 || true
  docker compose --project-name "$edge_project" --file deploy/compose.edge.yaml down --volumes --remove-orphans >/dev/null 2>&1 || true
  docker compose --project-name "$app_project" --file deploy/compose.app.yaml down --volumes --remove-orphans >/dev/null 2>&1 || true
  docker network rm "$EDGE_NETWORK" >/dev/null 2>&1 || true
  docker network rm "$PORTAINER_PROXY_NETWORK" >/dev/null 2>&1 || true
  rm -rf "$test_dir"
}
trap cleanup EXIT

docker network create "$EDGE_NETWORK" >/dev/null
docker network create --internal "$PORTAINER_PROXY_NETWORK" >/dev/null
docker compose --project-name "$portainer_project" --file deploy/compose.portainer.yaml config --quiet
docker compose --project-name "$app_project" --file deploy/compose.app.yaml config --quiet
docker compose --project-name "$edge_project" --file deploy/compose.edge.yaml config --quiet
docker compose --project-name "$app_project" --file deploy/compose.app.yaml up --detach --wait --wait-timeout 180
docker compose --project-name "$portainer_project" --file deploy/compose.portainer.yaml up --detach --wait --wait-timeout 180
docker compose --project-name "$edge_project" --file deploy/compose.edge.yaml up --detach --wait --wait-timeout 180

postgres_id="$(docker compose --project-name "$app_project" --file deploy/compose.app.yaml ps -q postgres)"
api_id="$(docker compose --project-name "$app_project" --file deploy/compose.app.yaml ps -q api)"
admin_id="$(docker compose --project-name "$app_project" --file deploy/compose.app.yaml ps -q admin)"
edge_id="$(docker compose --project-name "$edge_project" --file deploy/compose.edge.yaml ps -q edge)"
portainer_id="$(docker compose --project-name "$portainer_project" --file deploy/compose.portainer.yaml ps -q portainer)"

postgres_networks="$(docker inspect --format '{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}' "$postgres_id")"
api_networks="$(docker inspect --format '{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}' "$api_id")"
admin_networks="$(docker inspect --format '{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}' "$admin_id")"
edge_networks="$(docker inspect --format '{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}' "$edge_id")"
portainer_networks="$(docker inspect --format '{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}' "$portainer_id")"

[[ "$postgres_networks" == "$app_project"_app' ' ]]
[[ "$api_networks" == "$app_project"_app' ' ]]
[[ "$admin_networks" == *"$app_project"_app* && "$admin_networks" == *"$EDGE_NETWORK"* ]]
[[ "$edge_networks" == *"$EDGE_NETWORK"* && "$edge_networks" == *"$PORTAINER_PROXY_NETWORK"* ]]
[[ "$portainer_networks" == *"$PORTAINER_PROXY_NETWORK"* && "$portainer_networks" != *"$EDGE_NETWORK"* ]]

http_port="$(docker compose --project-name "$edge_project" --file deploy/compose.edge.yaml port edge 80)"
http_port="${http_port##*:}"
curl --fail --silent --show-error --header 'Host: localhost' "http://127.0.0.1:$http_port/" | grep -qi '<html'
curl --fail --silent --show-error --header 'Host: localhost' "http://127.0.0.1:$http_port/api/health/ready" | grep -q '"status":"ok"'
for attempt in {1..30}; do
  if curl --fail --silent --header 'Host: portainer.localhost' "http://127.0.0.1:$http_port/" | grep -qi 'portainer'; then
    break
  fi
  sleep 1
done
[[ "$attempt" -lt 30 ]]

printf '隔离部署的网络边界、管理端和 Portainer 域名路由验证通过\n'
