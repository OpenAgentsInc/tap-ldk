#!/usr/bin/env bash
set -euo pipefail

fork_url="https://github.com/OpenAgentsInc/rust-lightning.git"
base_rev="0c37f08a55c0f7738f2691dc3690166fd42f851d"
rev="15710fb516e800b2d3cb4e5d9d3525a4e573b24e"

remote_rev="$(git ls-remote "$fork_url" refs/heads/main | awk '{print $1}')"
if [[ "$remote_rev" != "$rev" ]]; then
  echo "OpenAgentsInc rust-lightning fork main is $remote_rev, expected $rev" >&2
  exit 1
fi

cargo metadata --format-version 1 \
  | grep -F 'git+https://github.com/OpenAgentsInc/rust-lightning?rev='"$rev" >/dev/null

echo "OpenAgentsInc rust-lightning fork is reachable at $rev and present in cargo metadata"
