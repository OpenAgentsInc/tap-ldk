# Polar Regtest Topology

Date: 2026-05-25

Polar is useful for the manual `lnd`/`tapd`/`litd` software interop side of the
demo and as a reference for the headless harness. It must not become a wallet
sidecar for `tap-ldk`.

## Source Build Status

Local source:

- path: `../projects/repos/polar`
- upstream: https://github.com/jamaljsr/polar
- local commit: `ee3ae493d613`
- Polar version: `4.0.0`
- local Node: `v25.8.2`
- local npm: `11.11.1`
- `yarn` was not installed directly on `PATH`.

Commands run from `../projects/repos/polar`:

```bash
npx -y yarn@1.22.22 --version
npx -y yarn@1.22.22 install --frozen-lockfile
npx -y yarn@1.22.22 tsc
npx -y yarn@1.22.22 build
```

Results:

- `npx -y yarn@1.22.22 --version` returned `1.22.22`.
- `yarn install --frozen-lockfile` completed.
- During install, optional dependency `cpu-features@0.0.9` failed to compile
  against Node 25/V8/NAN APIs, but Yarn treated it as optional and continued.
- `yarn tsc` passed.
- `yarn build` completed and produced the CRA production build.
- The production build warned that optional `ssh2` native modules
  `sshcrypto.node` and `cpu-features` were not resolvable. Those warnings are
  consistent with the optional native dependency build issue above.

Ignored build artifacts were removed after recording the build result; they are
not part of `tap-ldk` and must not be committed from the workspace root.

## Relevant Polar Node Versions

From Polar `docker/nodes.json`:

- Bitcoin Core latest: `30.0`
- LND latest: `0.20.0-beta`
- LND versions compatible with Bitcoin Core `30.0`: `0.20.0-beta`,
  `0.19.3-beta`, `0.19.2-beta`, `0.19.1-beta`, `0.19.0-beta`,
  `0.18.5-beta`, `0.18.4-beta`
- `tapd` latest: `0.7.0-alpha`
- `tapd` `0.7.0-alpha` compatibility: LND `0.19.0-beta`
- `litd` latest: `0.16.0-alpha`
- `litd` `0.16.0-alpha` compatibility: Bitcoin Core `30.0`

For the first Track B interop smoke, use either:

- Bitcoin Core `30.0` + LND `0.19.0-beta` + `tapd` `0.7.0-alpha`; or
- Bitcoin Core `30.0` + `litd` `0.16.0-alpha`.

The explicit LND/`tapd` pair is better for validating the daemon split. `litd`
is useful if the manual UI path is simpler, but the docs and demo should still
state which `lnd`/`tapd`/`litd` daemon surface is acting as the counterparty.

## Recommended Track B Manual Topology

Use Polar to operate only the `lnd`/`tapd`/`litd` side:

1. Start a Polar regtest network with one Bitcoin Core node.
2. Add one LND node and one `tapd` node attached to it, or one `litd` node if
   that path is more reliable for manual testing.
3. Mine enough blocks and fund the `lnd`/`tapd`/`litd` node through Polar.
4. Mint or import the demo asset on the `lnd`/`tapd`/`litd` side.
5. Export or synchronize the asset proof through the local proof/universe path
   expected by the `tap-ldk` harness.
6. Start the native `tap-ldk` wallet outside Polar.
7. Connect `tap-ldk` to the Polar-managed Bitcoin regtest backend and to the
   `lnd`/`tapd`/`litd` peer as an external counterparty.
8. Run the RFQ, invoice, payment, balance, and proof checks.

The key boundary is that Polar may manage Bitcoin Core plus `lnd`/`tapd`/`litd`
daemons, but `tap-ldk` must still implement Taproot Assets logic natively.

## Headless Harness Implications

The Rust/CI harness should reuse Polar as a reference for:

- Docker image selection;
- version compatibility;
- port and credential layout;
- mining/funding lifecycle;
- log collection;
- manual export/import shape.

The headless harness should not depend on the Polar Electron app. It should be
able to run Bitcoin Core and the needed `lnd`/`tapd`/`litd` counterparty containers
directly, then launch native `tap-ldk` processes against that network.
