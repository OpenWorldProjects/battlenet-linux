use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub account_id: String,
    pub battle_tag: Option<String>,
}
impl Account {
    pub fn validate(&self) -> Result<(), String> {
        if self.account_id.is_empty()
            || self.account_id.len() > 24
            || !self.account_id.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("Battle.net did not return a verified account identity.".into());
        }
        if self
            .battle_tag
            .as_ref()
            .is_some_and(|tag| tag.len() > 128 || tag.chars().any(char::is_control))
        {
            return Err("Invalid account profile response.".into());
        }
        Ok(())
    }
}

// This is a game login ticket, not a developer API OAuth access token.
pub struct Ticket(Zeroizing<String>);
impl Ticket {
    pub fn parse(input: &str) -> Result<Self, String> {
        let text = input.trim();
        let token = if text.contains("://") {
            let url = url::Url::parse(text).map_err(|_| "Invalid callback URL.")?;
            let legacy = url.scheme() == "http"
                && url.host_str() == Some("localhost")
                && url.port() == Some(0);
            let managed = url.scheme() == "bnet-linux-login"
                && url.host_str() == Some("callback")
                && url.port().is_none();
            if !legacy && !managed {
                return Err("Unrecognized Battle.net sign-in response.".into());
            }
            let mut values = url.query_pairs().filter(|(key, _)| key == "ST");
            let token = values
                .next()
                .ok_or("The callback has no ST ticket.")?
                .1
                .into_owned();
            if values.next().is_some() {
                return Err("Ambiguous login callback.".into());
            }
            Zeroizing::new(token)
        } else {
            Zeroizing::new(text.to_owned())
        };
        let b = token.as_bytes();
        let valid = (37..=128).contains(&b.len())
            && matches!(&b[..2], b"EU" | b"US" | b"KR" | b"CN")
            && b[2] == b'-'
            && b[35] == b'-'
            && b[3..35].iter().all(u8::is_ascii_hexdigit)
            && b[36..].iter().all(u8::is_ascii_digit);
        if !valid {
            return Err(
                "Blizzard returned an invalid sign-in response. Please try signing in again."
                    .into(),
            );
        }
        Ok(Self(token))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    #[allow(dead_code)] // Also compiled into the helper; matching is enforced by the parent.
    pub fn account_id(&self) -> &str {
        self.expose().rsplit('-').next().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TOKEN: &str = "EU-0123456789abcdef0123456789abcdef-12345";
    #[test]
    fn account_profile_requires_an_identity_and_never_accepts_control_characters() {
        assert!(
            Account {
                account_id: "12345".into(),
                battle_tag: Some("Fixture#1234".into())
            }
            .validate()
            .is_ok()
        );
        assert!(
            Account {
                account_id: String::new(),
                battle_tag: None
            }
            .validate()
            .is_err()
        );
        assert!(
            Account {
                account_id: "12345".into(),
                battle_tag: Some("name\nforged status".into())
            }
            .validate()
            .is_err()
        );
        assert_eq!(Ticket::parse(TOKEN).unwrap().account_id(), "12345");
    }
    #[test]
    fn accepts_ticket_and_exact_callback() {
        assert_eq!(Ticket::parse(TOKEN).unwrap().expose(), TOKEN);
        assert_eq!(
            Ticket::parse(&format!("http://localhost:0/?ST={TOKEN}"))
                .unwrap()
                .expose(),
            TOKEN
        );
        assert_eq!(
            Ticket::parse(&format!("bnet-linux-login://callback/?ST={TOKEN}"))
                .unwrap()
                .expose(),
            TOKEN
        );
    }
    #[test]
    fn rejects_bad_origin_ambiguous_and_malformed_tickets() {
        for value in [
            "",
            "EU-x-1",
            "http://example.com:0/?ST=x",
            "http://localhost:12/?ST=x",
            "bnet-linux-login://example.com:0/?ST=x",
            "other-app://localhost:0/?ST=x",
            "EU-0123456789abcdef0123456789abcdef-1\nmalicious",
        ] {
            assert!(Ticket::parse(value).is_err());
        }
        assert!(Ticket::parse(&format!("http://localhost:0/?ST={TOKEN}&ST={TOKEN}")).is_err());
    }
}
