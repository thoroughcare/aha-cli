//! Read/write a single `machine` entry in a netrc file.
//!
//! Standard netrc format: token stream, whitespace-separated, with `#`
//! line comments. A `machine HOST` token introduces a block; subsequent
//! `login` / `password` / `account` / `port` tokens belong to that block
//! until the next `machine` (or `default`) token.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// A single netrc machine entry. We only care about login + password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub host: String,
    pub login: String,
    pub password: String,
}

/// Default netrc path: `$HOME/.netrc`.
pub fn default_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME env var not set")?;
    Ok(PathBuf::from(home).join(".netrc"))
}

/// Find the entry for `host` in a netrc file, if present.
pub fn read(path: &Path, host: &str) -> Result<Option<Entry>> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    Ok(parse(&text)?.into_iter().find(|e| e.host == host))
}

/// Insert or replace the entry for `host`. Atomic: writes to a temp file
/// in the same directory, then renames over the target. Creates with
/// mode 0600 if the file is new; preserves existing mode on update.
pub fn upsert(path: &Path, entry: &Entry) -> Result<()> {
    let mut entries = read_all(path)?;
    if let Some(existing) = entries.iter_mut().find(|e| e.host == entry.host) {
        *existing = entry.clone();
    } else {
        entries.push(entry.clone());
    }
    write_atomic(path, &entries)
}

/// Remove the entry for `host`. No-op if not present or file doesn't exist.
pub fn remove(path: &Path, host: &str) -> Result<()> {
    let mut entries = read_all(path)?;
    let before = entries.len();
    entries.retain(|e| e.host != host);
    if entries.len() == before {
        return Ok(());
    }
    write_atomic(path, &entries)
}

fn read_all(path: &Path) -> Result<Vec<Entry>> {
    match fs::read_to_string(path) {
        Ok(t) => parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

fn parse(text: &str) -> Result<Vec<Entry>> {
    let mut tokens: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        tokens.extend(line.split_whitespace());
    }

    let mut entries = Vec::new();
    let mut cur: Option<PartialEntry> = None;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "machine" => {
                if let Some(pe) = cur.take() {
                    if let Some(e) = pe.into_entry() {
                        entries.push(e);
                    }
                }
                let host = tokens
                    .get(i + 1)
                    .context("netrc: 'machine' missing hostname")?;
                cur = Some(PartialEntry {
                    host: (*host).to_string(),
                    login: None,
                    password: None,
                });
                i += 2;
            }
            "default" => {
                if let Some(pe) = cur.take() {
                    if let Some(e) = pe.into_entry() {
                        entries.push(e);
                    }
                }
                cur = None;
                i += 1;
            }
            "login" => {
                let v = tokens.get(i + 1).context("netrc: 'login' missing value")?;
                if let Some(pe) = cur.as_mut() {
                    pe.login = Some((*v).to_string());
                }
                i += 2;
            }
            "password" | "account" => {
                let v = tokens
                    .get(i + 1)
                    .context("netrc: 'password' missing value")?;
                if tokens[i] == "password" {
                    if let Some(pe) = cur.as_mut() {
                        pe.password = Some((*v).to_string());
                    }
                }
                i += 2;
            }
            "port" | "macdef" => {
                // skip token + value
                i += 2;
            }
            _ => i += 1,
        }
    }
    if let Some(pe) = cur.take() {
        if let Some(e) = pe.into_entry() {
            entries.push(e);
        }
    }
    Ok(entries)
}

struct PartialEntry {
    host: String,
    login: Option<String>,
    password: Option<String>,
}

impl PartialEntry {
    fn into_entry(self) -> Option<Entry> {
        Some(Entry {
            host: self.host,
            login: self.login.unwrap_or_default(),
            password: self.password?,
        })
    }
}

fn render(entries: &[Entry]) -> String {
    let mut s = String::new();
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str(&format!(
            "machine {}\n  login {}\n  password {}\n",
            e.host, e.login, e.password
        ));
    }
    s
}

fn write_atomic(path: &Path, entries: &[Entry]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("creating parent dir {}", parent.display()))?;

    let existing_mode = file_mode(path);
    let rendered = render(entries);

    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("creating temp file in {}", parent.display()))?;
    tmp.write_all(rendered.as_bytes())
        .context("writing netrc temp file")?;
    tmp.flush().context("flushing netrc temp file")?;

    // Set perms BEFORE rename so the file is never world-readable on disk.
    let target_mode = existing_mode.unwrap_or(0o600);
    set_mode(tmp.path(), target_mode)?;

    tmp.persist(path)
        .map_err(|e| anyhow::anyhow!("renaming netrc temp file: {e}"))?;
    Ok(())
}

#[cfg(unix)]
fn file_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .ok()
        .map(|m| m.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn file_mode(_path: &Path) -> Option<u32> {
    None
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("chmod {} {:o}", path.display(), mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(host: &str, login: &str, password: &str) -> Entry {
        Entry {
            host: host.into(),
            login: login.into(),
            password: password.into(),
        }
    }

    #[test]
    fn parse_single_entry() {
        let text = "machine example.com\n  login alice\n  password s3cret\n";
        let parsed = parse(text).unwrap();
        assert_eq!(parsed, vec![entry("example.com", "alice", "s3cret")]);
    }

    #[test]
    fn parse_multiple_entries_and_comments() {
        let text = "\
            # leading comment\n\
            machine a.example login u1 password p1 # inline comment\n\
            machine b.example\n  login u2\n  password p2\n\
            default login d password dp\n\
            machine c.example login u3 password p3\n";
        let parsed = parse(text).unwrap();
        assert_eq!(
            parsed,
            vec![
                entry("a.example", "u1", "p1"),
                entry("b.example", "u2", "p2"),
                entry("c.example", "u3", "p3"),
            ]
        );
    }

    #[test]
    fn read_returns_none_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nope");
        assert_eq!(read(&path, "x").unwrap(), None);
    }

    #[test]
    fn upsert_creates_file_with_0600() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".netrc");
        upsert(&path, &entry("h.example", "oauth", "tok")).unwrap();

        let got = read(&path, "h.example").unwrap();
        assert_eq!(got, Some(entry("h.example", "oauth", "tok")));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn upsert_replaces_existing_entry_and_keeps_others() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".netrc");
        let initial = "\
            machine other.example login keep password keepme\n\
            machine target.example login old password oldtoken\n";
        fs::write(&path, initial).unwrap();

        upsert(&path, &entry("target.example", "oauth", "newtoken")).unwrap();

        let got = read(&path, "target.example").unwrap().unwrap();
        assert_eq!(got, entry("target.example", "oauth", "newtoken"));

        let other = read(&path, "other.example").unwrap().unwrap();
        assert_eq!(other, entry("other.example", "keep", "keepme"));
    }

    #[test]
    #[cfg(unix)]
    fn upsert_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join(".netrc");
        fs::write(&path, "machine x login a password b\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        upsert(&path, &entry("y", "u", "p")).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    #[test]
    fn remove_deletes_target_keeps_others() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".netrc");
        fs::write(
            &path,
            "machine a login u1 password p1\nmachine b login u2 password p2\n",
        )
        .unwrap();

        remove(&path, "a").unwrap();

        assert_eq!(read(&path, "a").unwrap(), None);
        assert_eq!(read(&path, "b").unwrap(), Some(entry("b", "u2", "p2")));
    }

    #[test]
    fn remove_is_noop_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(".netrc");
        remove(&path, "nope").unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn round_trip_preserves_long_token() {
        // Snowflake-shaped token similar to real Aha! API tokens.
        let token = "bu8_QytgVWODSXM75xLlRwiAGYipFEX8jSci9JAIUE0";
        let dir = tempdir().unwrap();
        let path = dir.path().join(".netrc");
        upsert(&path, &entry("tcare.aha.io", "oauth", token)).unwrap();
        let got = read(&path, "tcare.aha.io").unwrap().unwrap();
        assert_eq!(got.password, token);
    }
}
