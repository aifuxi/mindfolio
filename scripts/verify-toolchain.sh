#!/usr/bin/env bash
set -euo pipefail

[[ "$(mise --version | awk '{print $1}')" == "2026.6.14" ]]
[[ "$(rustc --version | awk '{print $2}')" == "1.97.1" ]]
[[ "$(node --version)" == "v24.21.0" ]]
[[ "$(pnpm --version)" == "12.3.4" ]]
cargo fmt --version >/dev/null
cargo clippy --version >/dev/null
