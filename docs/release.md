# Local release

The Flatpak bundles the Rust launcher, managed GTK/WebKit sign-in component, fonts, artwork, and embedded Windows bridge. GNOME Platform 50 supplies consistent GTK/WebKit libraries; the launcher is compiled inside Freedesktop SDK 25.08 rather than against this workstation's newer glibc. Wine, Proton/umu, game files, and existing prefixes remain on the host. The host helper forwards runtime configuration without replacing the host's desktop or graphics environment.

Build with `sh scripts/package-flatpak.sh`. Requires installed `org.freedesktop.Sdk//25.08`, `org.gnome.Platform//50`, Rust, cached Cargo dependencies, and host GTK3/WebKitGTK 4.1 development packages. Install the resulting file with `flatpak --user install --bundle dist/battlenet-linux-0.1.0-x86_64.flatpak`. Launch from the application menu or `flatpak run io.github.battlenet_linux.Launcher`.

The package has home/external game-directory access and permission to run host Wine/Proton through `flatpak-spawn`. It is intended for desktop portability, not isolation from the user's files. No Windows runtimes or game payloads are bundled.

## Integration status

StarCraft II online sign-in and launch were previously verified on this machine. This release adds native installation/update and launch adapters for StarCraft: Remastered, Heroes of the Storm, Overwatch, Warcraft I/II Remastered, Warcraft III: Reforged, WoW Retail/Classic/Classic Era/Anniversary, Hearthstone, Diablo II: Resurrected, Diablo III, and Diablo IV. Paths, Windows tags, and storage modes follow Blizzard's public product configurations.

At the user's request, newly added adapters have not undergone game downloads or launch testing. Compilation does not establish successful online authentication, compatibility with anti-cheat, or a playable installation. Encrypted metadata without a supported key, unfamiliar CASC formats, and changed layouts stop with an error. Server entitlements still apply. Experimental chat is implemented; account authorization and friend-list loading have been observed locally; message delivery remains unverified. See `docs/chat.md`.

WoW editions default to separate installation folders to avoid one edition replacing another. Existing shared installations can be selected in Game settings; product-specific metadata rows and executable subfolders distinguish editions. The same prefix/runtime can be reused across games.

Updates check the selected installed game at startup and periodically while idle. Cancel stops before activation; downloaded verified content can be reused. Cached downloads and append-only archives require additional disk space. Downloads are not automatically pruned in this release.

The tray supports Show, game selection, and Quit. Close-to-tray only hides the window after a StatusNotifier watcher is available; otherwise closing exits normally. Some desktops, including stock GNOME without a tray extension, do not provide a watcher.

The library uses official compact Blizzard game icons, including its favorites bar. Shop opens an in-app catalog. Product and website links open through the host default browser with error reporting. Launcher preferences and game settings use separate modal layouts; game settings group installation, runtime, updates, and authorization rather than presenting runtime diagnostics as the main flow.

Game settings now provide native folder/executable pickers, bounded discovery of installed games, Wine prefixes, and common Wine/Proton runners (Lutris, Steam/GE-Proton, umu, and Bottles). Selecting a game executable locates its installation and keeps the title's supported launch path; it does not bypass the required SC2/Heroes switcher. Wine and Proton selections preserve their inactive paths.

Scan & Repair streams checksums for indexed game data and loose install files, downloads replacements, and uses the existing staged/append-only update transaction. It targets the latest published build. Severely damaged CASC indexes/bootstrap metadata cannot currently be reconstructed automatically. The repair action has not been run against the user's game files during this change.

Chat and contacts remain visible in the right sidebar. Conversations can also be expanded. Game management actions remain available below the social panel.
