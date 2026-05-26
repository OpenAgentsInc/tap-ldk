# Tap-LDK Web Demo Application Specification

Date: 2026-05-26

## Purpose

Build a simple operator-facing web application that makes the current
`tap-ldk` demo understandable, inspectable, and configurable. The app should
show what is happening in Path A and Path B, let an operator create bounded
demo assets, run multiple local native `tap-ldk` instances, and exercise the
independent Lightning Labs counterparty path through a local Polar/headless
regtest topology.

The web app is a demo control plane and visualization layer. It must not become
the protocol authority, stablecoin issuer, wallet runtime, or a substitute for
the Rust implementation and smoke scripts.

## Current Source Of Truth

- Path A native-to-native demo: `scripts/path-a-native-demo.sh`.
- Path B Lightning Labs interop demo: `scripts/path-b-lightning-labs-demo.sh`.
- Full wrapper: `scripts/full-demo-smoke.sh`.
- Current machine-readable artifacts: JSON and proof files under `target/`.
- Current protocol and demo constraints: `INVARIANTS.md`, `ROADMAP.md`, and
  `docs/public-demo-runbook.md`.

The first web version should wrap these existing surfaces before inventing new
runtime behavior.

## Recommended Stack

- App framework: TanStack Start with React and TypeScript.
- Styling: Tailwind CSS, utility-first, with a compact dashboard UI.
- Host: Cloudflare Workers using Workers Static Assets for the built client and
  the TanStack Start server entry.
- Local execution bridge: a small Rust or TypeScript local demo agent that runs
  on the operator machine and exposes a narrow HTTP/WebSocket API for the web
  app.
- Realtime coordination: Cloudflare Durable Objects for demo-run rooms and
  WebSocket fan-out when the UI is deployed remotely.
- Persistence: D1 for run metadata and compact state; R2 for artifact bundles,
  logs, proof files, and screenshots; local filesystem for local-only runs.
- Background work: Cloudflare Queues only for non-local asynchronous tasks such
  as artifact upload, report summarization, or future remote-node workflows.

Reference notes:

- Cloudflare currently documents TanStack Start deployment on Workers and says
  Wrangler can detect TanStack Start, with generated output using a server entry
  and `.output/public` assets. See
  https://developers.cloudflare.com/workers/framework-guides/web-apps/tanstack-start/.
- Cloudflare recommends Workers Static Assets for new full-stack/static app
  deployments, rather than Workers Sites. See
  https://developers.cloudflare.com/workers/static-assets/.
- Cloudflare Durable Objects are the right fit for coordinating many WebSocket
  clients around one run room. See
  https://developers.cloudflare.com/durable-objects/best-practices/websockets/.
- TanStack Start server functions are same-origin RPC endpoints; public or
  cross-origin connector APIs should use explicit server routes. See
  https://tanstack.com/start/latest/docs/framework/react/guide/server-functions.
- Tailwind's TanStack Start guide uses `tailwindcss` plus
  `@tailwindcss/vite` and imports `@import "tailwindcss";` from the app CSS.
  See https://tailwindcss.com/docs/installation/framework-guides/tanstack-start.

## Execution Boundary

Cloudflare Workers cannot directly spawn `cargo`, Docker, Polar, `tapcli`,
`lncli`, or local `tap-ldk` binaries on the user's machine. The web app must
therefore use one of these transports:

1. Local development transport:
   - TanStack Start dev server or browser talks to
     `http://127.0.0.1:<port>` where the local demo agent is running.
   - The local agent shells out only through an allowlist of repo-owned
     commands and structured CLI subcommands.

2. Deployed Cloudflare transport:
   - Browser talks to the deployed Worker.
   - Worker routes the run to a Durable Object room.
   - The local demo agent opens an outbound WebSocket to that room.
   - Commands flow from the authenticated browser to the Durable Object to the
     paired local agent; events, logs, and artifacts flow back the same way.

3. Fixture/artifact transport:
   - UI loads an existing artifact directory or uploaded artifact bundle.
   - No local commands run.
   - This is the safe public demo/read-only mode.

All mutating execution must pass through the local agent. The browser and
Cloudflare Worker should not contain broad shell command execution.

## Product Scope

### MVP Scope

The MVP should support:

- Load and visualize the latest Path A or Path B artifact directory.
- Start a Path A run with configurable demo inputs.
- Start a Path B fixture/interoperability run and show its documented gaps.
- Start or connect to a local Polar/headless Lightning Labs topology when the
  local agent supports it.
- Create a bounded demo asset and use it in a Path A run.
- Run two to five local native `tap-ldk` instances in one demo run.
- Show node status, wallet balances, proof flow, channel state, RFQ/quote
  state, HTLC state, payment state, restart checks, close checks, and artifact
  paths.
- Make every mocked or deferred piece visible in the UI.

### Explicit Non-Goals For MVP

- No production issuance, reserves, redemption, compliance, or custody claim.
- No browser-held private wallet keys.
- No arbitrary command execution from the web UI.
- No claim that Path B live settlement is complete until the existing
  acceptance gate reports observed post-settlement balances.
- No Nostr-based public peer discovery in the MVP.
- No multi-asset-per-channel or multi-asset-per-HTLC UI unless the Rust demo
  supports it first.

## User Roles

- Local operator: runs demos on a developer machine, controls Docker/Polar,
  creates assets, and inspects logs/artifacts.
- Reviewer: opens a read-only run and verifies claims against generated
  artifacts.
- Future remote peer operator: pairs a local connector with a deployed run room
  to test with other nodes.

Authentication can be skipped for the first local-only version, but deployed
mode must require an operator session before it can send commands to a local
agent.

## Core User Workflows

### 1. Inspect An Existing Run

1. User selects an artifact directory or uploaded bundle.
2. App parses `summary.txt`, known JSON reports, logs, and proof files.
3. App shows:
   - demo path;
   - final claim status;
   - mocked/deferred items;
   - balances before/after;
   - proof and artifact inventory;
   - failed or blocked steps.

This mode must work without Docker, Polar, or a local agent if artifacts are
already present.

### 2. Run Native Path A

1. User chooses `Path A: local tap-ldk nodes`.
2. User configures asset and payment inputs.
3. App asks local agent to run a structured Path A command.
4. UI streams events:
   - regtest start;
   - wallet init;
   - asset issuance;
   - proof courier handoff;
   - channel funding;
   - RFQ and invoice binding;
   - HTLC add/settle;
   - restart verification;
   - cooperative close and proof export.
5. App stores the artifact path and renders the final report.

### 3. Run Local Multi-Instance Native Demo

1. User chooses two to five native nodes.
2. User creates or selects an asset.
3. User allocates starting balances.
4. User creates one or more channels from a bounded topology picker:
   - line;
   - star;
   - triangle for three nodes.
5. User sends one or more payments through the chosen path.
6. UI shows balance conservation and per-hop state transitions.

The first implementation may compile this into repeated existing CLI commands
and a new JSON run plan before adding a long-lived wallet daemon.

### 4. Run Path B With Lightning Labs Counterparty

1. User chooses `Path B: Lightning Labs litd counterparty`.
2. App verifies local agent, Docker, and Polar/headless topology readiness.
3. App starts or attaches to the configured local network.
4. App runs the current Path B wrapper.
5. UI renders:
   - standalone LND/`tapd` readiness;
   - integrated `litd` readiness;
   - live `tapd` proof binding;
   - fork-backed `ldk-node` to `litd` peer preflight;
   - fixture checks;
   - live settlement gate.
6. If Path B stops at the current known gap, the app labels it as blocked at
   live asset-channel payment settlement.

### 5. Create A Demo Asset

User controls:

- asset tag, default `OPENUSD`;
- display name;
- supply;
- decimal display;
- issuer key mode:
  - generated local demo key;
  - pasted public key;
  - fixture key;
- owner/script key target;
- metadata JSON;
- proof export mode:
  - native fixture proof;
  - `tapd` TAPF export if `tapd` is reachable;
  - local-only synthetic proof for Path A.

The UI must clearly label which proof path was used.

## UI Specification

The app should open directly into the demo console, not a marketing page.

### Global Layout

- Desktop: left sidebar navigation, top run selector/status strip, main work
  surface.
- Mobile/tablet: top bar with a menu button and a drawer for navigation.
- Primary sections:
  - Runs;
  - Network;
  - Assets;
  - Channels;
  - Payments;
  - Proofs;
  - Artifacts;
  - Settings.

### Main Run View

The run view should have three coordinated regions:

- Event timeline: ordered steps with status, duration, and linked artifacts.
- Network graph: nodes, channels, balances, and payment direction.
- Inspector panel: selected step JSON, logs, proof digest, or command report.

The view should be dense and operational. Avoid oversized hero copy, decorative
cards, and broad gradients. Use neutral surfaces, small status badges, compact
tables, and deterministic spacing.

### Visual Elements

- Channel graph:
  - native `tap-ldk` nodes;
  - Lightning Labs `litd` or LND/`tapd` nodes;
  - channel lines with asset balance labels;
  - animated payment path during replay.
- Balance ledger:
  - before and after values;
  - conservation check;
  - restart match;
  - observed versus expected balances.
- Proof flow:
  - issuance;
  - proof courier/import;
  - TAPF export/import;
  - proof digest;
  - final close proof.
- Compatibility matrix:
  - fixture-backed checks;
  - live checks;
  - documented gaps;
  - current issue/acceptance status.

### Controls

Use standard controls rather than custom novelty widgets:

- segmented control for Path A / Path B / Artifact Replay;
- tabs for run subviews;
- selects for topology, node count, asset source, counterparty mode;
- numeric inputs or steppers for supply, channel amount, payment amount, and
  oracle rate;
- toggles for restart check, cooperative close, live counterparty, and artifact
  upload;
- icon buttons with tooltips for replay, pause, reset, export, copy path, and
  open artifact.

### Status Language

Use explicit status labels:

- `not_started`
- `running`
- `passed`
- `failed`
- `blocked`
- `documented_gap`
- `deferred`

Do not label expected placeholders as successful live interop. Path B can show
green fixture checks while the live settlement gate remains blocked. The UI
must distinguish the current #57 pre-settlement readiness state from a
post-settlement observed-balance success.

## Local Demo Agent

The local agent is the only component allowed to interact with the local repo,
Docker, Polar, or native binaries.

### Responsibilities

- Discover repo root and current Git status.
- Report installed tool versions.
- Start/stop/status for:
  - Path A regtest Bitcoin;
  - native `tap-ldk` demo instances;
  - Lightning Labs standalone LND/`tapd`;
  - integrated `litd`;
  - Polar network when available.
- Run allowlisted scripts and CLI subcommands.
- Stream structured events, stdout/stderr summaries, and artifact paths.
- Redact credentials and tokens before emitting logs.
- Package artifact directories for upload or replay.

### Initial Allowlist

- `./scripts/path-a-native-demo.sh`
- `./scripts/path-b-lightning-labs-demo.sh`
- `./scripts/full-demo-smoke.sh`
- `./scripts/lightning-labs-counterparty.sh start|stop|status|ready|connection|smoke`
- `./scripts/lightning-labs-litd-counterparty.sh start|stop|status|ready|connection|smoke|balance`
- `cargo run -q -p tap-ldk-cli -- <known-demo-subcommand>`

Do not expose arbitrary shell strings. Commands should be represented as typed
actions with validated fields.

### Local Agent API

Example endpoints:

- `GET /health`
- `GET /environment`
- `GET /runs`
- `POST /runs`
- `GET /runs/:runId`
- `POST /runs/:runId/cancel`
- `GET /runs/:runId/events`
- `GET /runs/:runId/artifacts`
- `GET /artifacts/:artifactId`
- `POST /assets`
- `POST /nodes`
- `POST /channels`
- `POST /payments`
- `POST /polar/networks`
- `POST /connectors/cloudflare`

WebSocket event stream:

```json
{
  "run_id": "run_...",
  "sequence": 42,
  "time": "2026-05-26T12:00:00Z",
  "kind": "payment_settled",
  "status": "passed",
  "message": "native asset payment settled",
  "artifact_path": "target/.../native-payment.json",
  "data": {
    "asset_amount": 125,
    "sender_balance_after": 575,
    "receiver_balance_after": 425
  }
}
```

## Cloudflare App Architecture

### Request Routing

- TanStack Start routes render the dashboard and read-only reports.
- Server functions handle same-origin mutations from the UI.
- Public connector endpoints use explicit server routes, not server functions.
- A Durable Object named by run ID owns realtime state for each active run.
- D1 stores the compact run record.
- R2 stores large artifacts.

### Suggested Cloudflare Bindings

- `RUN_DB`: D1 database.
- `ARTIFACTS`: R2 bucket.
- `RUN_ROOMS`: Durable Object namespace.
- `EVENT_QUEUE`: optional Queue for async artifact post-processing.
- `ASSETS`: Workers Static Assets binding, if needed by the generated config.

### Data Model

Tables:

- `demo_runs`
  - `id`
  - `mode`
  - `status`
  - `created_at`
  - `updated_at`
  - `artifact_root`
  - `local_agent_id`
  - `summary_json`
- `demo_nodes`
  - `id`
  - `run_id`
  - `kind`
  - `label`
  - `node_pubkey`
  - `status`
- `demo_assets`
  - `id`
  - `run_id`
  - `asset_id`
  - `tag`
  - `supply`
  - `decimal_display`
  - `proof_source`
- `demo_channels`
  - `id`
  - `run_id`
  - `local_node_id`
  - `remote_node_id`
  - `asset_id`
  - `local_balance`
  - `remote_balance`
  - `status`
- `demo_payments`
  - `id`
  - `run_id`
  - `asset_id`
  - `amount`
  - `sender_node_id`
  - `receiver_node_id`
  - `status`
  - `observed_balance`
- `demo_events`
  - `id`
  - `run_id`
  - `sequence`
  - `kind`
  - `status`
  - `artifact_key`
  - `data_json`
- `artifact_objects`
  - `id`
  - `run_id`
  - `r2_key`
  - `kind`
  - `sha256`
  - `bytes`

## Server Function And Route Surface

Client-safe server functions:

- `listRuns()`
- `getRun({ id })`
- `createRun({ mode, config })`
- `cancelRun({ id })`
- `createDemoAsset({ runId, assetConfig })`
- `startNativeNode({ runId, nodeConfig })`
- `openAssetChannel({ runId, channelConfig })`
- `sendPayment({ runId, paymentConfig })`
- `closeChannel({ runId, channelId })`
- `exportArtifacts({ runId })`

Public/server routes:

- `GET /api/connector/:pairingCode/ws`
- `POST /api/connector/:pairingCode/events`
- `POST /api/artifacts/upload`
- `GET /api/runs/:id/events`

All inputs must be schema-validated. Use exact enum values for mode, path,
status, topology, and command action.

## Nostr / Future Network Extension

Nostr should be a later discovery and coordination adapter, not an MVP
dependency. The app should reserve a typed `PeerDiscoveryProvider` interface:

```ts
type PeerDiscoveryProvider =
  | { kind: "manual"; nodes: ManualPeer[] }
  | { kind: "local_registry"; registryUrl: string }
  | { kind: "nostr"; relays: string[]; eventKinds: number[] };
```

Future Nostr usage should be limited to signed announcements and connection
metadata:

- node identity;
- supported network;
- asset-channel feature support;
- endpoint hints;
- proof/universe service hints;
- run-room invitation or pairing code.

Do not publish private keys, macaroon material, wallet state, full proofs, or
raw local logs to relays. Any Nostr routing must feed a typed peer selector or
explicit parser, not ad hoc keyword matching.

## Safety And Invariants

- The web app must display whether a run is Path A, Path B fixture-backed,
  Path B live-ready, or Path B live-settled.
- The app must not show a settled Path B claim until the Rust artifacts record
  observed post-settlement balances from both sides.
- The local agent must redact RPC passwords, macaroons, bearer tokens, and raw
  secret material from logs.
- The local agent must reject commands outside its typed allowlist.
- Artifact replay mode must be read-only.
- The app must preserve current `tap-ldk` language that LND, `tapd`, `litd`,
  and Polar are independent counterparties or orchestration tools, not native
  wallet sidecars.
- Every UI action that changes protocol behavior must correspond to a Rust CLI,
  script, test, or documented model boundary.

## Implementation Plan

### Phase 0: Artifact Viewer

- Scaffold `apps/demo-web`.
- Add Tailwind and base dashboard shell.
- Implement artifact directory import in local dev.
- Render Path A and Path B summaries, known JSON reports, logs, and proof
  artifact inventory.
- Add Playwright smoke for artifact replay.

Exit condition: A saved `target/full-demo-smoke/<timestamp>` directory can be
loaded and visualized without running Docker.

### Phase 1: Local Agent And Path A Runner

- Add `crates/tap-ldk-demo-agent` or `apps/demo-agent`.
- Implement health, environment, run creation, event stream, and artifact
  listing.
- Wrap `scripts/path-a-native-demo.sh`.
- Add configurable asset inputs where existing CLI support exists.
- Render live Path A progress.

Exit condition: The user can run Path A from the web app and verify the same
final balances and close-proof artifacts as the script.

### Phase 2: Multi-Instance Native Demo

- Define a typed run plan for two to five native nodes.
- Add local registry and graph rendering.
- Add topology picker and per-payment controls.
- Back missing CLI functionality with small Rust commands rather than shell
  string composition.

Exit condition: The app can run a multi-node local native plan and prove asset
conservation across the displayed topology.

### Phase 3: Path B / Polar / Lightning Labs

- Add local agent adapters for existing headless scripts.
- Add Polar attachment/start support after the Polar topology is stable enough
  to automate.
- Render LND/`tapd`/`litd` readiness separately.
- Render live `tapd` proof binding and fork-backed `ldk-node` to `litd`
  preflight.
- Keep current live settlement blocked state visible until #80, #81, and
  #57 are done, keep the reverse direction blocked until #58 is done, and do
  not show Path B as complete until #59 replaces expected balances with
  observed balances.

Exit condition: The app can run the current Path B wrapper, show fixture checks
as passed, and show live settlement as blocked unless observed balances exist.

### Phase 4: Deployed Cloudflare Pairing

- Add Durable Object run rooms.
- Add local agent outbound WebSocket pairing.
- Add D1/R2 persistence.
- Add read-only shared run URLs.

Exit condition: A deployed Cloudflare app can coordinate a local agent run
without exposing the operator's local HTTP service directly to the internet.

### Phase 5: Network Discovery Adapter

- Add manual remote peer entry first.
- Add typed Nostr discovery provider only after manual remote peer tests work.
- Keep all remote node announcements and routing decisions explicit and
  reviewable.

Exit condition: Two operators can discover or enter peer connection material
without moving secrets through the discovery layer.

## Verification

Required checks for the app:

- `cargo test` for new Rust agent code.
- Unit tests for command allowlist validation and log redaction.
- Type tests or runtime schema tests for all API inputs and event envelopes.
- Playwright tests for:
  - artifact replay;
  - Path A run start;
  - blocked Path B live settlement display;
  - mobile navigation.
- Integration smoke that runs:
  - `./scripts/path-a-native-demo.sh`;
  - `./scripts/path-b-lightning-labs-demo.sh` when Docker/Polar dependencies
    are reachable;
  - artifact parser against `./scripts/full-demo-smoke.sh` output.

Do not use UI tests as evidence that protocol behavior is correct. Protocol
evidence remains in Rust tests, fixture checks, formal models, smoke scripts,
and generated artifacts.
