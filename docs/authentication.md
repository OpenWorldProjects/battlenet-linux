# Authentication and SC2 launch

The launcher signs in to the Battle.net account independently of games through the official account website in the native WebKit helper. The account endpoint verifies the authenticated session and returns only the numeric account ID and optional BattleTag. Passwords, MFA, cookies, and callback tickets are not printed in diagnostics.

The verified profile is saved as `web-session/account.json` with mode 0600 under private application data. Reopening restores the library without another interactive login. The browser session is persistent in a 0700 directory, with a private cookie database. Sign out removes the profile and browser session and clears the selected game's protected ticket. A remembered profile is not a server credential. At startup and hourly, a hidden WebKit account check verifies the saved cookies against Blizzard. A completed login redirect marks the session expired and offers sign-in again. Network errors leave the account available, show an inconclusive status, and retry after five minutes; they do not erase cookies or claim expiry. Checks are bounded to 20 seconds and cannot open popups. The account header distinguishes a saved identity from a verified session.

Game authorization is separate: Play checks the DPAPI-protected ticket inside Wine and reuses it only if its account ID matches the selected profile. The ticket prefix identifies the issuing login service; it does not select the play region. The bridge applies the configured game region and locale. A missing, mismatched, or locally stale ticket opens the managed game authorization flow. Successful renewal writes a private timestamp; cached tickets are reused for at most 24 hours. This is a local reuse policy, not a claim about Blizzard’s server-side expiration time. Renewal uses Blizzard’s game-login endpoint and existing cookies, not an invented OAuth refresh token. Game settings provides explicit authorization refresh for expiry or server rejection.

Wine transports fresh tickets through a private pipe. Proton uses a loopback-only connection authenticated by a fresh OS-generated 256-bit nonce. Tickets never appear in process arguments or launcher settings. The helper protects them with current-user Windows DPAPI, using the Battle.net SDK entropy, and writes the per-title launch options. Existing tickets are decrypted and checked inside Wine, without exporting plaintext to Linux.

## Verified launch behavior

Current SC2 builds need `Support64/SC2Switcher_x64.exe` startup setup. Launching the versioned game executable directly reproduced exit 47. The Rust bridge now starts the switcher and follows its SC2 child, reporting the game's actual exit even when the switcher exits successfully first. A disposable synthetic switcher/game integration test covers that handoff.

The installation was outdated (5.0.16.97563); Blizzard's live manifest required 5.0.17.98310 in both EU and US. After patching, the Rust bridge launched SC2 with the Windows Battle.net UI closed, and the game reached the signed-in online menu. The user confirmed that game login works. Account persistence and game-ticket storage are separate from the native patch backend.

Synthetic WebKit tests cover cookie reuse, hidden validation, explicit expiration redirects, and rejecting unsigned accounts. Live saved cookies were also checked and the server redirected to login, confirming that the installed session required renewal. Long-term ticket expiry, account switching, and other providers/regions still need broader testing. Additional game adapters are implemented but unverified; see release.md.

## Desktop compatibility

Login diagnostics use fixed lifecycle events and process status only. A desktop check reproduced a pre-page GTK abort caused by inherited VS Code Snap schema paths. Child processes discard those editor overrides while retaining normal system and Flatpak data directories. The default Wayland startup check then passed. `--check-login-window` exercises the same environment without visiting a website or accessing an account.
