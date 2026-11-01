# Account and game authentication

For EU, US and KR, the main sign-in now authenticates the native Battle.net chat connection first. Blizzard supplies the managed browser challenge; after completing that single sign-in, the chat service returns a verified account ID, which becomes the launcher account. The library and friends use that same authenticated connection. Own-account presence resolves the BattleTag. Reconnection checks the returned ID against the remembered account and rejects a different identity. The website-account flow remains available for CN, where native chat is not supported.

Startup restores the remembered identity and connects chat directly with the saved browser session. Website-profile verification is no longer a prerequisite for chat or game actions. When automatic chat is disabled or paused, optional website checks use a separate background job, so they cannot put Play into its busy state. Reauthentication clears the old chat error state and starts a new shared connection. Network connectivity failures are not treated as proof of cookie expiry.

The first chat/authentication handoff below is covered by synthetic cookie and identity checks; a noninteractive live check did not complete with the saved session, so interactive Blizzard sign-in and real message delivery still need confirmation. No chat messages were sent by diagnostics.

## Earlier implementation and storage details


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

## Packaged browser font initialization

A blank chat authorization window was reproduced inside the GNOME Flatpak runtime with an empty browser session. The WebKit content process consumed a CPU core in fontconfig before emitting its first navigation event; the host helper loaded the same public login URL. An isolated font configuration resolved the runtime failure: navigation started, redirected, committed, and finished, without disabling sandboxing or graphics acceleration. The package installs this configuration for login-helper children only. Desktop font settings and cookies are preserved. Login diagnostics record fixed page lifecycle events, never URLs. Slow loading and renderer termination now produce a visible status instead of leaving only a blank window.

## Session-only cookie handoff

WebKit's persistent cookie store does not retain session-only cookies when the account helper exits. A regression fixture reproduced this: the account signed in successfully, but the next helper lost its authenticated state. On a successful account/game/chat sign-in, the helper now retains session cookies from the allowed Blizzard login origins for at most one day in the existing private cookie database. Existing persistent cookies keep their original expiry; Secure/HttpOnly and other cookie attributes are preserved. This local retention does not extend server-side authorization. Sign out removes the database as before. The integration fixture checks account-to-chat cookie reuse and automatic callback return without showing another window. Chat allows a short silent handoff before showing an interactive login window if needed. Real Blizzard session revocation or required MFA can still demand sign-in.

## Native credential renewal

After an authenticated logon, the native client requests AuthenticationServer.GenerateWebCredentials (method 8), then requests renewal every 30 minutes. Reconnect supplies these opaque server-issued bytes in LogonRequest.cached_web_credentials (field 12). A rejected cache is removed and retried once without it; only then can Blizzard's browser challenge require interaction. Renewal errors do not disconnect an otherwise valid chat session, and a timed-out renewal retries after five minutes.

Renewal credentials are stored atomically with mode 0600 under the existing 0700 session directory, bound to account and region, and removed by Sign out. They are not passwords and are never logged or placed in arguments. Local reuse is capped at 30 days, independently of any earlier server-side revocation. Cache corruption, oversized data, wrong account/region, and invalid timestamps are rejected. The optional website check is now restricted to CN and cannot revoke EU/US/KR chat authentication.

The wire definitions and method ID were checked against the primary generated protocol definitions:
https://github.com/TrinityCore/TrinityCore/blob/master/src/server/proto/Client/authentication_service.pb.cc
https://github.com/superp00t/gophercraft/blob/master/bnet/bgs/protocol/authentication/v1/authentication_service.pb.go

Cache binding, bounds and private atomic writes pass local tests. Live renewal still needs an authenticated connection to seed the credential cache; the noninteractive live check reached Blizzard's browser challenge but did not complete authentication. The implementation must not be described as verified password-free renewal until that round trip succeeds.


## Login lifetime and repeated launches

A chat connection owns its sign-in helper. Dropping or replacing that connection cancels the helper; Linux also terminates login helpers when their parent exits. Pressing sign-in during an existing connection attempt preserves that attempt. Reopening the launcher activates its existing window through a private local socket instead of starting another UI and authentication flow. Startup serialization includes a short retry while the first process binds its socket.

Tests cover connection cancellation, termination of a waiting helper, and single-instance activation and lock release. These address concrete paths that could discard the first login result or leave an orphan password window. The user's exact two-password sequence has not been reproduced in an automated live account test. A renewal credential file was present after the reported successful connection; silent renewal across a later restart remains unverified.
