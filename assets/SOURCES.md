# Artwork sources

Official StarCraft II artwork retrieved from https://starcraft2.blizzard.com/en-us/ for this local interface prototype. Copyright Blizzard Entertainment; not project-owned artwork. Review distribution permissions before publishing bundles.

- `hero.png`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/bltd6a472d809e4de76/675814effcc31dc2bd387566/masthead_overview_background_1600.webp
- `logo.png`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt2f9554037b40a4de/69644b1df4fa3fa5af01a31f/logo.webp
- `races.png`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blte8484cc0fe5b46ef/696452c4c8f277406c43adb2/game_features_races.webp
- `coop.png`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt998dd9afaacbca3f/696452deb0cf6f853c26d709/game_features_f2p.webp
- `multiplayer.png`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt045232b3225ed22e/6a2c6b65c0223bc1a54b49e8/game_features_legendary_multiplayer.png

- `battlenet-logo.svg`: official Battle.net logo served by Blizzard’s login page: https://bnetaccount.akamaized.net/login/static/images/toolkit/defaults/logos/bnet/logo-vertical-color-light.1UZ9S.svg
- `battlenet-icon.svg` and `battlenet-horizontal.svg`: layouts using the unmodified paths from that official asset. PNGs are rendered from these SVGs for egui and the window icon.

- `fonts/Inter-Regular.otf` and `fonts/Inter-SemiBold.otf`: Inter by Rasmus Andersson, from the system fonts-inter package; SIL Open Font License 1.1, included in `fonts/LICENSE`. Upstream: https://rsms.me/inter/

- `starcraft-remastered.image`: official game-site social artwork: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt9cbae43d0642e6ba/6a9b42028102310ecb81e0e3/OG_Image.png

- `heroes.image`: official game-site social artwork: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt8c01f980b76eb350/67ce291560fcdc2c6cfed487/og_image.webp

- `game-icons/*.svg`: unmodified official Blizzard navigation icons from `https://forge.akamaized.net/icons/<filename>.svg`, discovered through `https://navbar.blizzard.com/static/v1/nav.js`. Matching PNGs are rendered at 96px for egui.
- `heroes-logo.png`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt834ba90f64d3e3cf/6a9804768102311ee181e07a/logo.png (reference asset, not used as the compact icon).
- `starcraft-logo.png`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt1ff010bc61d5b9eb/6aa1fbadf8f124a920bcba58/logo.png (reference asset, not used as the compact icon).

- Account portraits are fetched at runtime from Blizzard's public `https://blzprofile.akamaized.net/static/avatar/account/<avatarId>.jpg` catalogue; the selected numeric ID comes from account presence, never from a guessed BattleTag mapping. Manifest: https://blzprofile.akamaized.net/static/avatar/account/index.json. Portraits remain Blizzard assets and are cached locally rather than bundled.

## Game catalogue artwork

The following official website images are resized to at most 1200×900 and encoded as JPEG for bundling. WoW editions currently share the official WoW artwork.

- `game-art/S2.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/bltbe2068a317e02d9f/6966bac0fb2fd910dbda4a26/og_image.webp
- `game-art/S1.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt9cbae43d0642e6ba/6a9b42028102310ecb81e0e3/OG_Image.png
- `game-art/Hero.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt8c01f980b76eb350/67ce291560fcdc2c6cfed487/og_image.webp
- `game-art/Pro.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt2477dcaf4ebd440c/blt45586c965db08717/6823abc24dee72d806fff5e2/OpenGraph.jpg
- `game-art/W3.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt806d59ff0b4e0508/6aa88158ad9209ba7f5629ef/rebirth_og.png
- `game-art/WoW.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/bltb5a24e5ab1e2cfb0/6a88e3589b942efdb6f74110/wow-thumbnail-homepage.jpg
- `game-art/WoWC.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/bltb5a24e5ab1e2cfb0/6a88e3589b942efdb6f74110/wow-thumbnail-homepage.jpg
- `game-art/WoWE.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/bltb5a24e5ab1e2cfb0/6a88e3589b942efdb6f74110/wow-thumbnail-homepage.jpg
- `game-art/WoWA.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/bltb5a24e5ab1e2cfb0/6a88e3589b942efdb6f74110/wow-thumbnail-homepage.jpg
- `game-art/WTCG.jpg`: https://d39zum0jwvcigt.cloudfront.net/_next/static/images/default-475d770302527dbab7708dca2af05afd.jpg
- `game-art/D3.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blte3178c04d93773f1/67ce27f440e6651e27e17582/og_image.webp
- `game-art/Fen.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt7e63ffa7acc28b95/692f34c7826cf18bafcedc51/d4-open-graph_001.jpg?format=webp
- `game-art/OSI.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blt9c12f249ac15c7ec/blt185726ac34f99175/698100271279400c31d2c641/infernal_OG.webp
- `game-art/W1R.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blta8f9a8e092360c6c/bltef23deacbfc70dd3/671fd0f27c61fd5321468778/blizzard_game_card_background_wr.jpg
- `game-art/W2R.jpg`: https://blz-contentstack-images.akamaized.net/v3/assets/blta8f9a8e092360c6c/blt8d825cfe40ef2d21/671fd10ad32591ddef9e7606/blizzard_game_card_background_w2r.jpg
