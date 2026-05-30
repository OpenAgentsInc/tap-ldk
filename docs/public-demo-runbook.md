# Public Demo Runbook

Date: 2026-05-25

This runbook shows the current `tap-ldk` demo state. The demo proves bounded
native Rust/LDK Taproot Assets wallet functionality without using an LND/`tapd`
wallet sidecar. It does not prove production stablecoin issuance, redemption,
reserve management, compliance, live routing, or live on-chain force-close
recovery.

## Status

Path A is the runnable native-to-native demo. It issues a bounded local
`OPENUSD` asset, moves proof material between two native wallets, exercises
native asset-channel/payment/restart/close smokes, and exports cooperative
close proof artifacts. The recovery smoke also validates bounded force-close,
second-level HTLC, and final sweep proof-ownership records and refuses BTC-only
sweeps as asset recovery.

Path B is the native `tap-ldk` to independent `lnd`/`tapd`/`litd` compatibility
demo. It runs fixture-backed Lightning Labs software blob/proof/funding/RFQ
checks plus native payment-state checks, and optionally starts a Docker- or Podman-backed Bitcoin
Core/LND/`tapd` counterparty. LND and `tapd` are compatibility peers, not
`tap-ldk` runtime sidecars. The live gate now also starts integrated `litd`
for the asset-channel path, connects the fork-backed OpenAgentsInc `ldk-node`
runtime to that `litd` peer, funds a live asset channel, settles `litd` to
native, then sends the asset back from native LDK to `litd`. The latest
live report has `issue_81_acceptance_met=true`, `issue_57_acceptance_met=true`,
and `issue_58_acceptance_met=true` with no invalid-commitment or counterparty
force-close markers. The Path B wrapper completion report now has
`path_b_live_observed_balance_gate_met=true` and `live_daemon_gaps_remaining=false`
from observed live balances and `path_b_complete=true` after semantic proof
ancestry validation. #19 is closed for the first-demo Path B scope; future
production hardening should be tracked separately.

The first public Taproot Assets demo keeps one asset-channel funding outpoint
from open through payment, restart, close, and force-close. BTC-level
simple-taproot splice nonce maps are now covered in the fork; asset-channel
splice/RBF still needs separate proof and state coverage before a demo claim.

## Prerequisites

- Git, Bash, and standard Unix shell utilities.
- Rust `1.85` or newer. This runbook was last checked with
  `rustc 1.94.1` and `cargo 1.94.1`.
- Docker or Podman is optional. A container runtime is only needed for the
  independent `lnd`/`tapd`/`litd` Path B counterparty smoke.
- Path B counterparty target: Bitcoin Core `30.0`, LND `0.19.0-beta`,
  `tapd` `0.7.0-alpha`.

## Setup

```bash
git clone https://github.com/OpenAgentsInc/tap-ldk.git
cd tap-ldk
cargo fmt --check
cargo test
```

## Path A: Native-To-Native

Run:

```bash
./scripts/path-a-native-demo.sh
```

Artifacts are written to `target/path-a-native-demo/<timestamp>/`. Override the
location with:

```bash
TAP_LDK_PATH_A_ARTIFACT_DIR=/tmp/tap-ldk-path-a ./scripts/path-a-native-demo.sh
```

Expected output includes:

```text
path-a-native-demo: artifacts=...
Path A native-to-native demo artifacts: ...
- local wallets issue and courier OPENUSD proof material
- native asset channel funds at alice=700 bob=300
- native payment settles 125 OPENUSD to bob
- recovery smoke checks funding/RFQ/HTLC/commitment/settlement/close-prep restart boundaries
- cooperative close exports final proofs at alice=575 bob=425
- force-close proof-ownership recovery is machine-visible, while live on-chain
  sweeper integration remains pending
```

Key artifacts:

- `summary.txt`
- `alice-wallet.json`
- `bob-wallet.json`
- `bob-openusd-proof.tlv`
- `asset-channel-funding.json`
- `asset-commitment.json`
- `native-payment.json`
- `native-recovery.json`
- `native-close.json`
- `native-close-local-proof.hex`
- `native-close-remote-proof.hex`
- `close-recovery-status.json`
- `logs/`

Mocked or bounded pieces:

- Issuer identity is a bounded local demo key.
- Price oracle is fixed at `100` millisats per `OPENUSD` unit.
- Proof courier is a local file handoff.
- UI is the headless CLI smoke.
- Live on-chain sweeper integration remains pending. The bounded recovery
  smoke refuses BTC-only sweep state as asset recovery.

## Path B: `lnd`/`tapd`/`litd` Interop

Run:

```bash
./scripts/path-b-lightning-labs-demo.sh
```

Artifacts are written to `target/path-b-lightning-labs-demo/<timestamp>/`.
Override the location with:

```bash
TAP_LDK_PATH_B_ARTIFACT_DIR=/tmp/tap-ldk-path-b ./scripts/path-b-lightning-labs-demo.sh
```

Expected output includes:

```text
path-b-lightning-labs-demo: artifacts=...
Path B `lnd`/`tapd`/`litd` interop demo artifacts: ...
Independent counterparty:
- target: Bitcoin Core 30.0, LND 0.19.0-beta, tapd 0.7.0-alpha
Fixture-backed checks:
- blob fixtures: ...
- proof fixtures: ...
- funding interop: ...
- RFQ invoice compatibility: ...
- tap-ldk pays `litd` artifacts: ...
- `litd` pays tap-ldk artifacts: ...
- consolidated checks: ...
```

If no container runtime is available, or if the selected runtime is not
running, expected output also includes a dependency gap. The harness now checks
the Docker Desktop app bundle CLI before falling back to Podman:

```text
Neither Docker nor Podman is installed. Path B fixture-backed checks ran, but
the independent `lnd`/`tapd` counterparty was not started.

<runtime> is installed at <path>, but its daemon or machine is not reachable.
```

Key artifacts:

- `summary.txt`
- `versions.txt`
- `lightning-labs-counterparty-config.json`
- `lightning-labs-counterparty-gap.txt`
- `live-tap-ldk-peer.json`
- `live-tapd-proof-binding.json`
- `lightning-labs-blob-fixtures.json`
- `lightning-labs-proof-fixtures.json`
- `lightning-labs-funding-interop-report.json`
- `lightning-labs-rfq-invoice.json`
- `lightning-labs-outgoing-payment-report.json`
- `lightning-labs-incoming-payment-report.json`
- `lightning-labs-interop-checks.json`
- `logs/`

When the integrated `litd` runtime is reachable, the consolidated report now
requires live observed balances and should report
`live_daemon_gaps_remaining=false`. If the container runtime or daemon is not
reachable, the report must show a dependency gap instead of claiming live Path
B success.

Mocked or bounded pieces:

- Issuer identity and price oracle remain bounded demo fixtures.
- Proof courier is local typed-bundle import/export plumbing.
- Manual/local discovery is used for the first interop target.
- Live `tap-ldk` peer smoke is local `tap-ldk` to `tap-ldk`; it is not yet a
  `lnd`/`tapd`/`litd` daemon-backed P2P session.
- Live `tapd` proof binding can bind daemon-exported proof material when the
  `lnd`/`tapd`/`litd` runtime is reachable.
- Fork-backed `ldk-node` can connect to integrated `litd`, reach the Taproot
  Asset message/channel/payment APIs, and complete both live settlement
  directions for the first demo.
- Live post-close proof export, force-close/sweep recovery, and network proof
  universe/courier behavior remain open.

## Full Smoke Wrapper

Run both paths and collect one artifact tree:

```bash
./scripts/full-demo-smoke.sh
```

Artifacts are written to `target/full-demo-smoke/<timestamp>/`, with Path A
under `path-a/`, Path B under `path-b/`, and wrapper logs under `logs/`.
Override the location with:

```bash
TAP_LDK_FULL_DEMO_ARTIFACT_DIR=/tmp/tap-ldk-full ./scripts/full-demo-smoke.sh
```

Expected output:

```text
full-demo-smoke: artifacts=...
Full tap-ldk demo smoke artifacts: ...
Path A artifacts:
- .../path-a
Path B artifacts:
- .../path-b
Logs:
- .../logs/path-a-native-demo.out
- .../logs/path-a-native-demo.err
- .../logs/path-b-lightning-labs-demo.out
- .../logs/path-b-lightning-labs-demo.err
```

## Verification

Before presenting demo results publicly, run:

```bash
cargo fmt --check
cargo test
cargo run -p tap-ldk-cli -- live-peer-smoke target/live-peer-smoke.json 7a3811630bb33503c6536c3a223d3caecb93fe55f4b3439528edf27b10d38e93
./scripts/path-a-native-demo.sh
./scripts/path-b-lightning-labs-demo.sh
./scripts/full-demo-smoke.sh
```

Report Path A and Path B separately. The #59 completion report is now
`target/path-b-lightning-labs-demo-issue59/path-b-completion-report.json`; it
refuses fixture-only or expected-only balance completion and, after #60, may
set `path_b_complete=true` when the live observed-balance gate stays green.
Keep the completed #81, #57, #58, #59, and #60 gates green as regressions.
