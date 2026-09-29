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
