use crate::runner::{self, Game};
use std::{io::Read, time::Duration};

#[derive(Clone, Debug, PartialEq)]
pub struct Version {
    pub name: String,
    pub build: String,
}
#[derive(Clone, Debug)]
pub struct Status {
    pub installed: String,
    pub latest: Version,
}
impl Status {
    pub fn needs_update(&self) -> bool {
        self.installed != self.latest.build
    }
}
pub fn manifest_version(text: &str, region: &str) -> Result<Version, String> {
    let mut lines = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty());
    let header: Vec<_> = lines
        .next()
        .ok_or("Empty Blizzard version manifest.")?
        .split('|')
        .collect();
    let column = |name| {
        header
            .iter()
            .position(|field| field.split('!').next() == Some(name))
            .ok_or("Invalid Blizzard version manifest.")
    };
    let region_column = column("Region")?;
    let name_column = column("VersionsName")?;
    let build_column = column("BuildId")?;
    for line in lines {
        let fields: Vec<_> = line.split('|').collect();
        if !fields
            .get(region_column)
            .is_some_and(|value| value.eq_ignore_ascii_case(region))
        {
            continue;
        }
        let name = fields.get(name_column).ok_or("Missing Blizzard version.")?;
        let build = fields.get(build_column).ok_or("Missing Blizzard build.")?;
        if build.is_empty()
            || build.len() > 12
            || !build.bytes().all(|c| c.is_ascii_digit())
            || name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        {
            return Err("Invalid Blizzard build metadata.".into());
        }
        return Ok(Version {
            name: (*name).into(),
            build: format!("Base{build}"),
        });
    }
    Err("Blizzard did not publish a build for this region.".into())
}
pub fn installation_key(game: &Game) -> String {
    let stamp = runner::install_root(game)
        .ok()
        .and_then(|root| std::fs::metadata(root.join(".build.info")).ok())
        .and_then(|metadata| metadata.modified().ok());
    format!("{}|{}|{stamp:?}", game.executable, game.region)
}
pub fn check(game: &Game) -> Result<Status, String> {
    let product = crate::catalog::product(&game.product_code)?;
    let installed =
        runner::installed_build(game).ok_or("Cannot determine the installed game build.")?;
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?
        .get(format!(
            "https://eu.version.battle.net/{}/versions",
            product.slug
        ))
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("Could not check Blizzard versions: {e}"))?;
    let mut text = String::new();
    response
        .take(65537)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 65536 {
        return Err("Blizzard version manifest is too large.".into());
    }
    Ok(Status {
        installed,
        latest: manifest_version(&text, &game.region)?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    const MANIFEST: &str = "Region!STRING:0|BuildId!DEC:4|VersionsName!String:0\n## seqn = 1\nus|98310|5.0.17.98310\neu|97563|5.0.16.97563\n";
    #[test]
    fn selects_requested_region_instead_of_first_row() {
        assert_eq!(
            manifest_version(MANIFEST, "EU").unwrap(),
            Version {
                name: "5.0.16.97563".into(),
                build: "Base97563".into()
            }
        );
        assert!(manifest_version(MANIFEST, "KR").is_err());
    }
    #[test]
    fn rejects_invalid_or_incomplete_manifests() {
        assert!(manifest_version("Region|BuildId|VersionsName\neu|../bad|5.0.17", "EU").is_err());
        assert!(manifest_version("Region|BuildId|VersionsName\neu|98310", "EU").is_err());
        assert!(manifest_version("unrelated response", "EU").is_err());
    }
}
