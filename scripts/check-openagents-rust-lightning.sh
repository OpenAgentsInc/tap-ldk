#!/usr/bin/env bash
set -euo pipefail

fork_url="https://github.com/OpenAgentsInc/rust-lightning.git"
base_rev="0c37f08a55c0f7738f2691dc3690166fd42f851d"
rev="99fee582d4061af4b0a030353b0a409ee542e064"

remote_rev="$(git ls-remote "$fork_url" refs/heads/main | awk '{print $1}')"
if [[ "$remote_rev" != "$rev" ]]; then
  echo "OpenAgentsInc rust-lightning fork main is $remote_rev, expected $rev" >&2
  exit 1
fi

cargo metadata --format-version 1 \
  | grep -F '"https://github.com/OpenAgentsInc/rust-lightning.git' >/dev/null

echo "OpenAgentsInc rust-lightning fork is reachable at $rev and present in cargo metadata"
