use crate::runner::Game;
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, Debug)]
pub struct Product {
    pub code: &'static str,
    pub slug: &'static str,
    pub name: &'static str,
    pub folder: &'static str,
    pub executable: &'static str,
    pub process: &'static str,
    pub data_dir: &'static str,
    pub website: &'static str,
    pub auth_code: &'static str,
    pub game_dir: &'static str,
    pub containerless: bool,
}
pub const PRODUCTS: [Product; 15] = [
    Product {
        code: "S2",
        auth_code: "S2",
        game_dir: "",
        containerless: false,
        slug: "s2",
        name: "StarCraft II",
        folder: "StarCraft II",
        executable: "Support64/SC2Switcher_x64.exe",
        process: "SC2_x64.exe",
        data_dir: "SC2Data",
        website: "https://starcraft2.blizzard.com/",
    },
    Product {
        code: "S1",
        auth_code: "S1",
        game_dir: "",
        containerless: false,
        slug: "s1",
        name: "StarCraft: Remastered",
        folder: "StarCraft",
        executable: "x86_64/StarCraft.exe",
        process: "StarCraft.exe",
        data_dir: "Data",
        website: "https://starcraft.blizzard.com/",
    },
    Product {
        code: "Hero",
        auth_code: "Hero",
        game_dir: "",
        containerless: false,
        slug: "hero",
        name: "Heroes of the Storm",
        folder: "Heroes of the Storm",
        executable: "Support64/HeroesSwitcher_x64.exe",
        process: "HeroesOfTheStorm_x64.exe",
        data_dir: "HeroesData",
        website: "https://heroesofthestorm.blizzard.com/",
    },
    Product {
        code: "Pro",
        slug: "pro",
        name: "Overwatch",
        folder: "Overwatch",
        executable: "_retail_/Overwatch.exe",
        process: "Overwatch.exe",
        data_dir: "data/casc",
        website: "https://overwatch.blizzard.com/",
        auth_code: "Pro",
        game_dir: "_retail_",
        containerless: false,
    },
    Product {
        code: "W3",
        slug: "w3",
        name: "Warcraft III: Reforged",
        folder: "Warcraft III",
        executable: "_retail_/x86_64/Warcraft III.exe",
        process: "Warcraft III.exe",
        data_dir: "Data",
        website: "https://warcraft3.blizzard.com/",
        auth_code: "W3",
        game_dir: "_retail_",
        containerless: false,
    },
    Product {
        code: "W1R",
        slug: "w1r",
        name: "Warcraft I: Remastered",
        folder: "Warcraft I Remastered",
        executable: "x86/Warcraft.exe",
        process: "Warcraft.exe",
        data_dir: "Data",
        website: "https://warcraft.blizzard.com/",
        auth_code: "W1R",
        game_dir: "",
        containerless: true,
    },
    Product {
        code: "W2R",
        slug: "w2r",
        name: "Warcraft II: Remastered",
        folder: "Warcraft II Remastered",
        executable: "x86/Warcraft II.exe",
        process: "Warcraft II.exe",
        data_dir: "Data",
        website: "https://warcraft.blizzard.com/",
        auth_code: "W2R",
        game_dir: "",
        containerless: true,
    },
    Product {
        code: "WoW",
        slug: "wow",
        name: "World of Warcraft",
        folder: "World of Warcraft",
        executable: "_retail_/WoW.exe",
        process: "WoW.exe",
        data_dir: "Data",
        website: "https://worldofwarcraft.blizzard.com/",
        auth_code: "WoW",
        game_dir: "_retail_",
        containerless: false,
    },
    Product {
        code: "WoWC",
        slug: "wow_classic",
        name: "WoW Classic",
        folder: "World of Warcraft Classic",
        executable: "_classic_/WoWClassic.exe",
        process: "WoWClassic.exe",
        data_dir: "Data",
        website: "https://worldofwarcraft.blizzard.com/",
        auth_code: "WoW",
        game_dir: "_classic_",
        containerless: false,
    },
    Product {
        code: "WoWE",
        slug: "wow_classic_era",
        name: "WoW Classic Era",
        folder: "World of Warcraft Classic Era",
        executable: "_classic_era_/WoWClassic.exe",
        process: "WoWClassic.exe",
        data_dir: "Data",
        website: "https://worldofwarcraft.blizzard.com/",
        auth_code: "WoW",
        game_dir: "_classic_era_",
        containerless: false,
    },
    Product {
        code: "WoWA",
        slug: "wow_anniversary",
        name: "WoW Classic Anniversary",
        folder: "World of Warcraft Anniversary",
        executable: "_anniversary_/WoWClassic.exe",
        process: "WoWClassic.exe",
        data_dir: "Data",
        website: "https://worldofwarcraft.blizzard.com/",
        auth_code: "WoW",
        game_dir: "_anniversary_",
        containerless: false,
    },
    Product {
        code: "WTCG",
        slug: "hsb",
        name: "Hearthstone",
        folder: "Hearthstone",
        executable: "Hearthstone.exe",
        process: "Hearthstone.exe",
        data_dir: "Data",
        website: "https://hearthstone.blizzard.com/",
        auth_code: "WTCG",
        game_dir: "",
        containerless: true,
    },
    Product {
        code: "D3",
        slug: "d3",
        name: "Diablo III",
        folder: "Diablo III",
        executable: "x64/Diablo III64.exe",
        process: "Diablo III64.exe",
        data_dir: "Data",
        website: "https://diablo3.blizzard.com/",
        auth_code: "D3",
        game_dir: "",
        containerless: false,
    },
    Product {
        code: "Fen",
        slug: "fenris",
        name: "Diablo IV",
        folder: "Diablo IV",
        executable: "Diablo IV.exe",
        process: "Diablo IV.exe",
        data_dir: "Data",
        website: "https://diablo4.blizzard.com/",
        auth_code: "Fen",
        game_dir: "",
        containerless: false,
    },
    Product {
        code: "OSI",
        slug: "osi",
        name: "Diablo II: Resurrected",
        folder: "Diablo II Resurrected",
        executable: "D2R.exe",
        process: "D2R.exe",
        data_dir: "Data",
        website: "https://diablo2.blizzard.com/",
        auth_code: "OSI",
        game_dir: "",
        containerless: false,
    },
];
pub fn product(code: &str) -> Result<&'static Product, String> {
    PRODUCTS
        .iter()
        .find(|p| p.code == code)
        .ok_or_else(|| "Unsupported game product.".into())
}
pub fn target_root(game: &Game) -> Result<PathBuf, String> {
    let p = product(&game.product_code)?;
    let root = if !game.install_dir.is_empty() {
        PathBuf::from(&game.install_dir)
    } else {
        let executable = Path::new(&game.executable);
        let mut root = executable;
        for _ in Path::new(p.executable).components() {
            root = root.parent().ok_or("Invalid installation path.")?;
        }
        root.to_path_buf()
    };
    let text = root.to_str().ok_or("Game path must be valid UTF-8.")?;
    if !root.is_absolute()
        || text.contains(['\n', '\r', '\0', '"'])
        || text.encode_utf16().count() > 400
    {
        return Err("Choose an absolute game installation path without quotes.".into());
    }
    Ok(root)
}
pub fn profiles(template: &Game, saved: Vec<Game>) -> Vec<Game> {
    let parent = target_root(template)
        .ok()
        .and_then(|p| p.parent().map(Path::to_owned))
        .or_else(|| directories::UserDirs::new().map(|d| d.home_dir().join("Games")))
        .unwrap_or_else(|| PathBuf::from("/tmp/Games"));
    PRODUCTS
        .iter()
        .map(|p| {
            let mut game = saved
                .iter()
                .find(|g| g.product_code == p.code)
                .cloned()
                .unwrap_or_else(|| {
                    if template.product_code == p.code {
                        return template.clone();
                    }
                    let root = parent.join(p.folder);
                    Game {
                        name: p.name.into(),
                        product_code: p.code.into(),
                        executable: root.join(p.executable).to_string_lossy().into(),
                        install_dir: root.to_string_lossy().into(),
                        ..template.clone()
                    }
                });
            game.name = p.name.into();
            if let Ok(root) = target_root(&game) {
                let expected = root.join(p.executable);
                if !Path::new(&game.executable).is_file() && expected.is_file() {
                    game.executable = expected.to_string_lossy().into();
                }
            }
            game
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_keep_independent_install_paths_and_shared_runtime() {
        let template = Game {
            executable: "/home/test/Games/StarCraft II/Support64/SC2Switcher_x64.exe".into(),
            prefix: "/home/test/prefix".into(),
            ..Game::default()
        };
        let profiles = profiles(&template, vec![]);
        assert_eq!(profiles.len(), PRODUCTS.len());
        assert_eq!(
            target_root(&profiles[1]).unwrap(),
            PathBuf::from("/home/test/Games/StarCraft")
        );
        assert_eq!(profiles[2].prefix, template.prefix);
        assert!(product("unknown").is_err());
    }
}
