# syntax=docker/dockerfile:1.7

FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a AS builder
COPY --from=ghcr.io/jdx/mise:2026.6.14@sha256:b8f8c20fc3308f8b1d00ccca2bc968e4e208af1c5c1069e1ad9753baa099acff /usr/local/bin/mise /usr/local/bin/mise
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential ca-certificates curl git pkg-config xz-utils \
    && rm -rf /var/lib/apt/lists/*
ENV MISE_DATA_DIR=/opt/mise
WORKDIR /workspace
COPY mise.toml mise.lock Cargo.toml Cargo.lock package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/api ./apps/api
COPY apps/admin ./apps/admin
COPY packages/api-contract ./packages/api-contract
COPY scripts/verify-toolchain.sh scripts/contract.mjs ./scripts/
RUN mise trust && mise run build:release

FROM debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a AS api
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /workspace/target/release/mindfolio-api /usr/local/bin/mindfolio-api
COPY --from=builder /workspace/target/release/migrate /usr/local/bin/migrate
COPY --from=builder /workspace/target/release/admin /usr/local/bin/admin
ARG VCS_REF=unknown
ARG IMAGE_VERSION=unknown
LABEL org.opencontainers.image.source="https://github.com/aifuxi/mindfolio" \
      org.opencontainers.image.revision="$VCS_REF" \
      org.opencontainers.image.version="$IMAGE_VERSION"
ENV API_BIND=0.0.0.0:3001
USER 10001:10001
EXPOSE 3001
HEALTHCHECK --interval=10s --timeout=3s --start-period=30s --retries=6 CMD ["curl", "--fail", "--silent", "--output", "/dev/null", "http://127.0.0.1:3001/health/ready"]
ENTRYPOINT ["/usr/local/bin/mindfolio-api"]

FROM caddy:2.10.2-alpine@sha256:4c6e91c6ed0e2fa03efd5b44747b625fec79bc9cd06ac5235a779726618e530d AS admin
COPY --from=builder /workspace/apps/admin/dist /srv/admin
COPY deploy/admin-container.Caddyfile /etc/caddy/Caddyfile
RUN mkdir -p /data /config && chown -R 10001:10001 /data /config /srv/admin /etc/caddy
ARG VCS_REF=unknown
ARG IMAGE_VERSION=unknown
LABEL org.opencontainers.image.source="https://github.com/aifuxi/mindfolio" \
      org.opencontainers.image.revision="$VCS_REF" \
      org.opencontainers.image.version="$IMAGE_VERSION"
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=6 CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8080/__health"]
