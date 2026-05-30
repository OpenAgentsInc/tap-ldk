#!/usr/bin/env bash
set -euo pipefail

cd /workspace

apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  ca-certificates \
  clang \
  cmake \
  curl \
  docker.io \
  git \
  jq \
  libssl-dev \
  lld \
  openjdk-17-jre-headless \
  pkg-config \
  protobuf-compiler \
  python3
rm -rf /var/lib/apt/lists/*

rustup component add rustfmt

if ! git rev-parse --show-toplevel >/dev/null 2>&1; then
  git init -q
  git config user.email "ci@openagents.com"
  git config user.name "OpenAgents CI"
  git add -A
  git commit -qm "cloudbuild workspace snapshot"
fi

if [ -z "${TLA_TOOLS_JAR:-}" ]; then
  TLA_TOOLS_JAR="/tmp/tla2tools.jar"
  if curl -fsSL -o "$TLA_TOOLS_JAR" \
    "https://github.com/tlaplus/tlaplus/releases/latest/download/tla2tools.jar"; then
    export TLA_TOOLS_JAR
  else
    rm -f "$TLA_TOOLS_JAR"
    unset TLA_TOOLS_JAR
    echo "cloudbuild-proof-engine: TLA+ tools download failed; formal-check will skip if no tlc is available."
  fi
fi

if [ "${TAP_LDK_EXTENDED_CHECKS:-0}" = "1" ]; then
  mkdir -p .worktrees
  if [ ! -d ".worktrees/rust-lightning/.git" ]; then
    git clone --filter=blob:none https://github.com/OpenAgentsInc/rust-lightning.git .worktrees/rust-lightning
  fi
  git -C .worktrees/rust-lightning fetch --depth 1 origin 3db3229733b724f45e7a356d923715213cb4f269
  git -C .worktrees/rust-lightning checkout -q FETCH_HEAD
  export OPENAGENTS_RUST_LIGHTNING_DIR="/workspace/.worktrees/rust-lightning"
  export TAP_LDK_RUST_LIGHTNING_DIR="/workspace/.worktrees/rust-lightning"
fi

./scripts/proof-engine-check.sh
