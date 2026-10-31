# Battle.net Linux

Unofficial Rust desktop client replacing the Battle.net launcher UI while running Windows games through Wine. Includes adapters for Blizzard games with host Wine/Proton runtimes.

StarCraft II account sign-in and online launch have been verified with the Windows Battle.net UI closed. The launcher remembers the signed-in profile across restarts. Native SC2 updates use Blizzard's CDN directly, with verified downloads, staged executable files, append-only CASC storage, cancellation, and interrupted-commit recovery. Additional game adapters are implemented; see [release status](docs/release.md) for their unverified status.

## Install release

Build with `sh scripts/package-flatpak.sh`, then run `sh scripts/install-release.sh` to install locally and preserve existing settings/sign-in. See [packaging and integration status](docs/release.md).

## Run

```sh
sh scripts/run.sh
```

The run script builds the launcher and its required sign-in component together. GTK 3 and WebKitGTK 4.1 development packages are needed for the sign-in component. The egui launcher uses OpenGL with Wayland and X11 support; it does not link GTK/WebKit itself.

Account sign-in opens Blizzard's account website in the managed login window, verifies the signed-in account through its account API, and returns automatically to the library. It does not require any game installation or Wine prefix. There is no callback field or URL-copying step. The login window supports retries and related sign-in popups. If it cannot complete sign-in, Play remains locked and the user can retry.

Check the installed game against Blizzard's current manifest with `cargo run --locked -- --profile launcher.local.json --check-updates`.

Import a local configuration with `cargo run --locked -- --profile launcher.local.json`. Use `--profile launcher.local.json --check-session` to check local ticket decryption without starting SC2. Local profiles contain paths and preferences only.

For Proton, fill the Proton directory and umu-run executable in Settings; the Wine field is used when Proton is empty.

Browse without signing in: `cargo run --locked -- --library`.

## Connect and launch SC2

1. Open **Game settings**. Set your existing Wine prefix (contains `system.reg`), Wine executable, and `StarCraft II/Support64/SC2Switcher_x64.exe`. Use absolute paths without quotes or `~`.
2. **Log in with Battle.net** signs into the account independently of games. The managed login window displays Blizzard's official account website. Passwords and MFA stay in that site. Game regions are selected in game settings.
3. **Play** first checks the protected game ticket inside Wine. It reuses a ticket only when its account ID matches the verified account, then applies the selected play region and locale. The token's region prefix identifies the issuing login service, not the selected play region. Otherwise it authorizes the selected game using the saved browser session. **Game settings → Refresh game authorization** explicitly replaces an expired ticket. A game ticket must match the verified account ID. The game-specific callback passes this ticket to the Rust Windows helper running in Wine: through stdin for Wine, or a nonce-authenticated loopback connection for Proton/umu. The helper protects it with Windows DPAPI and writes SC2's launch options.
4. Verified account profiles and the private browser session are remembered across restarts. Reopening the launcher restores the library; **Sign out** clears the remembered profile and browser session. A remembered profile alone cannot authorize game access: the game still needs its protected ticket.
5. **Play** starts SC2 through `Support64/SC2Switcher_x64.exe` using `-sso=1 -launch -uid s2`. The bridge follows the switcher's SC2 child and reports the game's actual exit rather than the switcher's successful exit. Current SC2 builds require the switcher's startup setup; directly invoking the versioned game binary can exit 47.
6. The launcher checks Blizzard's HTTPS version manifest in the background at startup and every 30 minutes. **Check for updates** in Game settings checks immediately. The main action changes to **Update** when required, and the installed build is checked again after patching. The installation must match Blizzard's live build for the selected region. An older build can open the game but fail login with **Version mismatch**; switching regions does not update the installation.

A game ticket is never treated as an account login. The launcher verifies the account independently. `--check-session` is diagnostic only. Use `--login` to open the official login window immediately.

**Ticket ready** means a correctly shaped ticket can be decrypted locally. It does not mean Blizzard accepted it or that the account is online. If SC2 rejects login, close the game and sign in again. No BattleTag, friends, or online status is fabricated.

**Sign out** removes this launcher's saved web session and attempts to remove the selected game's `WEB_TOKEN` from its prefix. It does not revoke server sessions or log out a running game. Wine DPAPI is a compatibility mechanism; protect the prefix as account data. Use a dedicated prefix and close the official client while testing to prevent competing writes.

## Current limits

- Version checks run at startup, every 30 minutes, and when installed build metadata changes. **Update** runs the native Rust patcher; it does not open Windows Battle.net or require its Agent. **Apply game updates automatically when idle** is enabled by default and can be changed in Game settings. Running SC2 processes and competing updaters prevent patch application.
- Native installation and updates support CASC and containerless NGDP product layouts. Newly added games have not been launch-tested, as requested. The Shop tab provides an in-app game catalog with external store links. Experimental Battle.net chat supports server authorization, contacts, and private messages; see [chat status](docs/chat.md). Owned-library lookup and live news are not implemented.
- Wine and existing Proton installations through umu are supported. No automatic runtime download or prefix creation.
- Launch requires Wine's default `Z:` mapping for absolute Unix paths.
- Status tracks the launch process through the runtime; normal closure and nonzero exit codes are reported separately. Closing this app leaves the game running.
- SC2 online launch is verified on the current installation. Other account/provider combinations, token expiry, and wider desktop compatibility need further testing.

Settings controls interface scale. Game settings stores paths, region, and locale through eframe, without login tickets. Launch details shows logs in a separate window. The browser session is kept in a private, user-only `web-session` directory under the application data directory; passwords are not stored by the launcher. Safe helper lifecycle events and process exit codes are recorded in `login-status.log`; callback URLs, tickets, cookies, and emails are excluded. Launch details show bounded game/runtime logs. Login operations use a separate runtime log, so they do not overwrite the last game launch diagnostics.

## Rebuild the Rust Windows helper

An embedded PE is included. Rebuild from source with Rust's bundled linker; no Windows SDK or C compiler is required:

```sh
rustup target add x86_64-pc-windows-msvc
sh scripts/build-bridge.sh
cargo build --locked
```

For deployment, put `bnet-login` beside the main binary. Development also detects `login-helper/target/debug/bnet-login`.

## Validation

```sh
cargo test --locked
cargo fmt --check
cargo fmt --manifest-path bridge/Cargo.toml --check
cargo fmt --manifest-path login-helper/Cargo.toml --check
xvfb-run -a python3 scripts/test-bridge.py
xvfb-run -a python3 scripts/test-login.py
```

The bridge test uses a disposable prefix, synthetic tickets, and a tiny test game that validates its working directory/data path and deliberately returns exit 47, plus a switcher fixture that exits 0 before its game exits 47: malformed input, DPAPI storage/decryption, missing-ticket launch rejection, failed spawn, removal, and the authenticated loopback transport. The WebKit test covers HTTP redirects, JavaScript navigation, form submission, and popup returns using synthetic tickets. Neither signs into a real account or proves SC2 online play.

## Source

- `src/main.rs`: session state, settings, background jobs, process status.
- `src/ui.rs`: shared typography, login, game library, artwork, and launch controls.
- `src/auth.rs`: shared callback/ticket validation.
- `src/runner.rs`: Wine invocation, ticket handoff, diagnostics.
- `src/updates.rs`: background Blizzard version checks.
- `src/patcher/`: native NGDP downloads, BLTE verification, CASC updates, staging, and recovery.
- `bridge/`: Rust Win32 helper for DPAPI, registry, and process APIs.
- `login-helper/`: managed WebKit sign-in with a private persistent browser session.

See [authentication evidence and remaining checks](docs/authentication.md) and [artwork sources](assets/SOURCES.md). This is an unofficial project, not affiliated with Blizzard Entertainment.

Desktop startup can be checked without a network request or account using `cargo run --locked -- --check-login-window`. Native child processes discard inherited editor/Snap GTK module and schema overrides, retaining system and Flatpak data paths. This prevents a reproduced Wayland GTK abort caused by a missing `antialiasing` schema key.

## Native update checks

`--profile launcher.local.json --plan-update` reads Blizzard's manifests and the installation, verifies loose-file hashes, and reports required downloads without applying a patch. Completed verified CDN files are cached for retry. New data archives and index generations preserve the old build; executable backups and a durable transaction journal allow recovery if application stops during activation. The build marker is published last.

The debug integration exercises `--exercise-native-update <new-directory>` and `--exercise-native-update-cancel <new-directory>` create independent sparse test installations. They download a real CASC blob and loose executable file, verify native activation/cancellation, and check that the source installation remains unchanged. These fixtures are not playable game copies. See [native updater details](docs/updates.md).
