# Native game updater

The updater is Rust code in `src/patcher/`; patch downloads and application do not use the Windows Battle.net client or its Agent. Version checks use Blizzard's product-specific HTTPS `versions` manifests at startup, every 30 minutes, and after local build metadata changes. Existing installation tags preserve the installed Windows assets and languages.

The planner verifies build/CDN configuration hashes, reads install/download/encoding manifests, locates changed payloads in CDN archives, and reuses existing CASC entries and verified cache files. Downloads run concurrently with archive-range checks and BLTE header/chunk MD5 verification. Decoded executable files must match their install-manifest content hashes and sizes.

New CASC payloads go into new archive files. Existing archives are never overwritten. New index generations retain existing resident entries, including journal updates and segment-header bucket assignments. CASC v4/v5 shared-memory records publish the new indexes and free-space state. Loose files are staged before activation; original files are backed up and a durable recovery journal records the transaction. `.build.info` is replaced last. A later launch/update recovers an interrupted loose-file commit before proceeding.

Cancellation is available before activation. Completed verified downloads remain cached for retry. The launcher avoids patching while SC2 is running, uses an installation update lock, detects a competing Windows updater, and rechecks the installation before activation. Automatic idle updates are configurable in Game settings.

## Validation

- A native read-only plan matched the real, patched SC2 installation: zero changed storage/loose files and zero download bytes.
- A separate sparse fixture removed one CASC entry and one executable file, then fetched both from Blizzard and verified archive/index/shared-memory updates, executable replacement, and final build activation.
- Cancellation in another fixture preserved the old build marker and missing-file state.
- Unit tests cover corrupt BLTE rejection, tag selection, path traversal rejection, index checksums, and interrupted/finished transaction recovery.

Sparse integration fixtures copy public install metadata and selected executable files and contain only necessary old archive payloads. They are isolated test data, not complete playable game copies. The original game installation is checked for changes after each exercise.

## Scope

Existing Windows StarCraft II installations are supported. Fresh installation, other products, encrypted metadata formats, binary-delta optimization, and archive garbage collection are not implemented. Unsupported formats or failed integrity checks abort rather than activating an unverified build. Old archive/index generations are retained, so updates can temporarily increase disk use.

The shared planner now selects each product’s CASC data directory and Windows tags. Shared-container games place loose files in their edition subfolder. Containerless games stage their install-manifest files directly, using a private metadata store instead of writing CASC archives into the game’s Data directory. New game adapters are unverified at the user’s request; see [release status](release.md).
