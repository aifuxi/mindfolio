#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

image_platform="${IMAGE_PLATFORM:-linux/amd64}"
api_image="${IMAGE_API_TAG:-mindfolio-api:local}"
admin_image="${IMAGE_ADMIN_TAG:-mindfolio-admin:local}"
edge_image="${IMAGE_EDGE_TAG:-mindfolio-edge:local}"
compose_file="deploy/compose.image-verify.yaml"

build_images() {
  local revision version target image
  revision="$(git rev-parse HEAD)"
  version="${IMAGE_VERSION:-sha-${revision:0:12}}"

  for target in api admin edge; do
    if [[ "$target" == api ]]; then
      image="$api_image"
    elif [[ "$target" == admin ]]; then
      image="$admin_image"
    else
      image="$edge_image"
    fi
    if [[ "$target" == edge ]]; then
      docker buildx build --load --platform "$image_platform" \
        --file deploy/Dockerfile.edge \
        --build-arg "VCS_REF=$revision" --build-arg "IMAGE_VERSION=$version" \
        --tag "$image" .
    else
      docker buildx build --load --platform "$image_platform" \
        --file deploy/Dockerfile --target "$target" \
        --build-arg "VCS_REF=$revision" --build-arg "IMAGE_VERSION=$version" \
        --tag "$image" .
    fi
  done
}

verify_images() {
  docker image inspect "$api_image" "$admin_image" "$edge_image" >/dev/null
  docker run --rm --platform "$image_platform" "$edge_image" caddy list-modules | grep -qx 'dns.providers.alidns'
  docker run --rm --platform "$image_platform" \
    --mount "type=bind,source=$repo_root/deploy/edge.Caddyfile,target=/etc/caddy/Caddyfile,readonly" \
    --env ADMIN_DOMAIN=http://admin.localhost \
    --env ALIYUN_ACCESS_KEY_ID=verify-only \
    --env ALIYUN_ACCESS_KEY_SECRET=verify-only \
    "$edge_image" caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
  project_id="mindfolio-image-verify-$$"
  export IMAGE_API_TAG="$api_image"
  export IMAGE_ADMIN_TAG="$admin_image"
  export IMAGE_TEST_DB_PASSWORD
  IMAGE_TEST_DB_PASSWORD="$(openssl rand -hex 24)"

  cleanup() {
    docker compose --project-name "$project_id" --file "$compose_file" down --volumes --remove-orphans >/dev/null
  }
  trap cleanup EXIT

  docker compose --project-name "$project_id" --file "$compose_file" up --detach --wait --wait-timeout 180
  docker compose --project-name "$project_id" --file "$compose_file" exec -T admin \
    wget -q -O - http://127.0.0.1:8080/ | grep -qi '<html'
  docker compose --project-name "$project_id" --file "$compose_file" exec -T admin \
    wget -q -O - http://127.0.0.1:8080/api/health/ready | grep -q '"status":"ok"'
  docker compose --project-name "$project_id" --file "$compose_file" exec -T api /usr/local/bin/migrate
  printf '生产镜像与同源代理验证通过\n'
}

publish_images() {
  local repository revision tag target image name digest summary source
  [[ "${GITHUB_EVENT_NAME:-}" == push && "${GITHUB_REF:-}" == refs/heads/master ]] || {
    printf '只允许 master 的 push 流水线发布镜像\n' >&2
    exit 1
  }
  [[ -n "${GHCR_TOKEN:-}" && -n "${GITHUB_ACTOR:-}" && -n "${GITHUB_REPOSITORY:-}" ]] || {
    printf '缺少 GHCR 发布环境变量\n' >&2
    exit 1
  }
  repository="$(printf '%s' "$GITHUB_REPOSITORY" | tr '[:upper:]' '[:lower:]')"
  revision="$(git rev-parse HEAD)"
  [[ "$revision" == "${GITHUB_SHA:-}" ]] || {
    printf '工作区提交与流水线提交不一致\n' >&2
    exit 1
  }
  tag="sha-$revision"
  printf '%s' "$GHCR_TOKEN" | docker login ghcr.io --username "$GITHUB_ACTOR" --password-stdin >/dev/null
  summary="${GITHUB_STEP_SUMMARY:-/dev/null}"
  {
    printf '### GHCR 镜像\n\n'
    printf '| 服务 | 版本 | 不可变引用 |\n| --- | --- | --- |\n'
  } >> "$summary"

  for target in api admin edge; do
    if [[ "$target" == api ]]; then
      image="$api_image"
    elif [[ "$target" == admin ]]; then
      image="$admin_image"
    else
      image="$edge_image"
    fi
    name="ghcr.io/$repository-$target"
    docker tag "$image" "$name:$tag"
    docker push "$name:$tag"
    digest="$(docker buildx imagetools inspect --format '{{.Manifest.Digest}}' "$name:$tag")"
    [[ "$digest" == sha256:* ]] || {
      printf '无法读取 %s 的发布 digest\n' "$name" >&2
      exit 1
    }
    printf '| %s | `%s` | `%s@%s` |\n' "$target" "$tag" "$name" "$digest" >> "$summary"
    printf '%s@%s\n' "$name" "$digest"
  done

  for target in postgres portainer; do
    if [[ "$target" == postgres ]]; then
      source="postgres:17.7-alpine@sha256:bb377b7239d2774ac8cc76f481596ce96c5a6b5e9d141f6d0a0ee371a6e7c0f2"
    else
      source="portainer/portainer-ce:2.45.1@sha256:4d616db18cfeb5dd41a69c0958bc825c84483ea9cde1106eb82a5d26f3bd8b0e"
    fi
    name="ghcr.io/$repository-$target"
    docker pull --platform "$image_platform" "$source"
    docker tag "$source" "$name:$tag"
    docker push "$name:$tag"
    digest="$(docker buildx imagetools inspect --format '{{.Manifest.Digest}}' "$name:$tag")"
    [[ "$digest" == sha256:* ]] || {
      printf '无法读取 %s 的发布 digest\n' "$name" >&2
      exit 1
    }
    printf '| %s（上游镜像副本） | `%s` | `%s@%s` |\n' "$target" "$tag" "$name" "$digest" >> "$summary"
    printf '%s@%s\n' "$name" "$digest"
  done
}

case "${1:-}" in
  build) build_images ;;
  verify) verify_images ;;
  publish) publish_images ;;
  *)
    printf '用法：%s {build|verify|publish}\n' "$0" >&2
    exit 2
    ;;
esac
