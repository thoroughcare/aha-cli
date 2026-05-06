pub mod netrc;

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub subdomain: String,
    pub token: String,
}

impl Credentials {
    pub fn host(&self) -> String {
        format!("{}.aha.io", self.subdomain)
    }

    pub fn base_url(&self) -> String {
        format!("https://{}/api/v1", self.host())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Overrides {
    pub subdomain: Option<String>,
    pub token: Option<String>,
}

/// Resolve credentials in priority order: CLI flags / env > netrc.
/// CLI flag values are pre-merged with env vars by `clap` (the `env = ...`
/// attribute on the global flags), so we receive a single `Overrides`.
pub fn resolve(overrides: &Overrides) -> Result<Credentials> {
    if let (Some(sub), Some(tok)) = (&overrides.subdomain, &overrides.token) {
        return Ok(Credentials {
            subdomain: sub.clone(),
            token: tok.clone(),
        });
    }

    let path = netrc::default_path()?;

    // If a subdomain is supplied, look up just that host.
    if let Some(sub) = &overrides.subdomain {
        let host = format!("{sub}.aha.io");
        if let Some(entry) = netrc::read(&path, &host)? {
            return Ok(Credentials {
                subdomain: sub.clone(),
                token: overrides.token.clone().unwrap_or(entry.password),
            });
        }
        return Err(missing_creds_err(Some(sub)));
    }

    // No subdomain hint — look for any *.aha.io entry written by us.
    if let Some(entry) = first_aha_entry(&path)? {
        let subdomain = entry
            .host
            .strip_suffix(".aha.io")
            .ok_or_else(|| anyhow!("netrc host {} not in .aha.io", entry.host))?
            .to_string();
        return Ok(Credentials {
            subdomain,
            token: overrides.token.clone().unwrap_or(entry.password),
        });
    }

    Err(missing_creds_err(None))
}

fn first_aha_entry(path: &std::path::Path) -> Result<Option<netrc::Entry>> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    // Crude: parse all entries, return first .aha.io. Avoids exporting the
    // internal parser API; netrc files are tiny.
    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("machine ") {
            let host = rest.split_whitespace().next().unwrap_or("");
            if host.ends_with(".aha.io") {
                return netrc::read(path, host);
            }
        }
    }
    Ok(None)
}

fn missing_creds_err(subdomain: Option<&str>) -> anyhow::Error {
    let target = subdomain
        .map(|s| format!(" for {s}.aha.io"))
        .unwrap_or_default();
    anyhow!(
        "no Aha! credentials found{target}. Run `aha auth login --subdomain <name>` \
         (or pass --token / set AHA_TOKEN)."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn host_and_base_url() {
        let c = Credentials {
            subdomain: "tcare".into(),
            token: "t".into(),
        };
        assert_eq!(c.host(), "tcare.aha.io");
        assert_eq!(c.base_url(), "https://tcare.aha.io/api/v1");
    }

    #[test]
    fn resolves_from_overrides() {
        let creds = resolve(&Overrides {
            subdomain: Some("foo".into()),
            token: Some("tok".into()),
        })
        .unwrap();
        assert_eq!(creds.subdomain, "foo");
        assert_eq!(creds.token, "tok");
    }

    #[test]
    fn resolves_from_netrc_via_first_aha_entry() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".netrc");
        std::fs::write(
            &path,
            "machine other.example login a password b\n\
             machine tcare.aha.io login oauth password tok123\n",
        )
        .unwrap();

        // Direct test of the helper (resolve() reads HOME, harder to isolate).
        let entry = first_aha_entry(&path).unwrap().unwrap();
        assert_eq!(entry.host, "tcare.aha.io");
        assert_eq!(entry.password, "tok123");
    }
}
