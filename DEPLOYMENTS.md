# OurDAO Contract Deployments

This document tracks the OurDAO DAO contract deployments across networks. Each deployment is linked to a specific GitHub release.

## Network Deployments

### Stellar Testnet

| Release | Contract ID | Status | Deployed |
|---------|---|---|---|
| Latest | TBD | Preparing | - |

### Stellar Public Network

| Release | Contract ID | Status | Deployed |
|---------|---|---|---|
| Latest | TBD | Not yet deployed | - |

## Verification

To verify a deployed contract matches a GitHub release:

1. **Find the release** — Go to [Releases](https://github.com/ourdao/ourdao-contracts/releases) and select the version.

2. **Download the wasm** — Each release includes `ourdao_dao.optimized.wasm`.

3. **Verify the checksum** — Compare the SHA-256 hash in the release notes against your copy:
   ```bash
   sha256sum ourdao_dao.optimized.wasm
   ```

4. **Confirm the contract ID** — Query the network for the contract you deployed:
   ```bash
   stellar contract info --id <CONTRACT_ID> --network testnet
   ```

## Reproducible build verification

The repository includes `scripts/build-reproducible.sh`, which builds
`ourdao-dao` inside a pinned Rust/Stellar CLI build container and prints the
SHA-256 hash of the resulting WASM.

Run a container-only build with:

```bash
./scripts/build-reproducible.sh
```

To prove that a local/CI artifact is byte-for-byte identical, pass that WASM as
the first argument:

```bash
stellar contract build \
  --locked \
  --package ourdao-dao \
  --optimize=false \
  --out-dir target/reproducible-native

./scripts/build-reproducible.sh \
  target/reproducible-native/ourdao_dao.wasm
```

The script exits non-zero if the hashes differ. CI performs this comparison on
every pull request, so a source revision is only considered reproducible when
the native reference build and the container build produce the same bytes.

The reproducible image is built from `Dockerfile.reproducible`, pinned to
Rust 1.98.1 and Stellar CLI 28.0.0. The local image tag defaults to
`ourdao-reproducible:rust-1.98.1-stellar-28.0.0`; override only the tag with
`REPRO_IMAGE` if required:

```bash
REPRO_IMAGE=ourdao-reproducible:verification \
  ./scripts/build-reproducible.sh target/reproducible-native/ourdao_dao.wasm
```

For release verification, record the printed SHA-256 in the release notes and
compare it with the downloaded release artifact before deployment.

## Deployment Process

To deploy a new release:

1. **Check the release** — Open the [latest release](https://github.com/ourdao/ourdao-contracts/releases) and verify the checksum.

2. **Download or build locally** — Either download the wasm from the release, or build it yourself:
   ```bash
   git checkout v<VERSION>
   stellar contract build --optimize
   ```

3. **Deploy** — Use the deployment script (for testnet) or a key-holding CI job (for production):
   ```bash
   ./scripts/deploy-testnet.sh
   ```

4. **Record the contract ID** — Update this file with the new deployment, including the release version and timestamp.

## Release History

- **v1.0.0** (not yet released) — Initial release candidate

## Notes

- Each release produces a reproducible, verifiable wasm artifact with a documented SHA-256.
- Contract IDs are recorded per network to enable members to verify they have the correct code.
- There is currently no upgrade path; a new deployment is a fresh contract with a new ID.