#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$ROOT" ]; then
  echo "gcloud-proof-engine-submit: unable to find repository root; run from inside the repo." >&2
  exit 1
fi
cd "$ROOT"

MODE="${1:-fast}"
case "$MODE" in
  fast)
    EXTENDED="0"
    shift || true
    ;;
  extended)
    EXTENDED="1"
    shift || true
    ;;
  -h|--help)
    cat <<USAGE
Usage: scripts/gcloud-proof-engine-submit.sh [fast|extended] [gcloud builds submit args...]

Submits cloudbuild.yaml to Google Cloud Build.

Modes:
  fast      Run ./scripts/proof-engine-check.sh with normal checks.
  extended Run the same wrapper with TAP_LDK_EXTENDED_CHECKS=1.

Environment:
  GOOGLE_CLOUD_PROJECT overrides the active gcloud project.
  GOOGLE_CLOUD_REGION overrides the Cloud Build region.
USAGE
    exit 0
    ;;
  *)
    echo "gcloud-proof-engine-submit: unknown mode '$MODE'." >&2
    echo "Use 'fast' or 'extended'." >&2
    exit 2
    ;;
esac

PROJECT="${GOOGLE_CLOUD_PROJECT:-$(gcloud config get-value project 2>/dev/null)}"
if [ -z "$PROJECT" ]; then
  echo "gcloud-proof-engine-submit: no Google Cloud project configured." >&2
  echo "Set GOOGLE_CLOUD_PROJECT or run: gcloud config set project <project-id>" >&2
  exit 1
fi

REGION="${GOOGLE_CLOUD_REGION:-$(gcloud config get-value builds/region 2>/dev/null || true)}"
if [ -z "$REGION" ]; then
  REGION="$(gcloud config get-value compute/region 2>/dev/null || true)"
fi

ARGS=(
  builds submit .
  --project "$PROJECT"
  --config cloudbuild.yaml
  --substitutions "_TAP_LDK_EXTENDED_CHECKS=$EXTENDED"
)

if [ -n "$REGION" ]; then
  ARGS+=(--region "$REGION")
fi

gcloud "${ARGS[@]}" "$@"
