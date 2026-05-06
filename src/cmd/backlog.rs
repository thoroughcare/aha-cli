//! `aha backlog` — group features by release → epic → status. The "browse
//! the roadmap" view nothing else gives us from the terminal.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;

use crate::client::models::Feature;
use crate::client::resources::FeatureFilters;
use crate::client::AhaClient;
use crate::output::OutputFormat;

pub async fn run(client: &AhaClient, filters: FeatureFilters, format: OutputFormat) -> Result<()> {
    let features = client.list_features(&filters).await?;
    let grouped = group(&features);
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&grouped)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&grouped)?);
        }
        OutputFormat::Table => render_table(&grouped),
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct GroupedBacklog {
    pub releases: Vec<ReleaseGroup>,
}

#[derive(Debug, Serialize)]
pub struct ReleaseGroup {
    pub release_ref: String,
    pub release_name: String,
    pub epics: Vec<EpicGroup>,
}

#[derive(Debug, Serialize)]
pub struct EpicGroup {
    pub epic_ref: String,
    pub epic_name: String,
    pub features: Vec<FeatureLine>,
}

#[derive(Debug, Serialize)]
pub struct FeatureLine {
    pub reference_num: String,
    pub name: String,
    pub status: String,
    pub complete: bool,
    pub assignee: Option<String>,
}

const NONE_RELEASE_REF: &str = "(no release)";
const NONE_EPIC_REF: &str = "(no epic)";

/// `(reference_num, name)`. We key on the pair so the rendered headers
/// stay aligned with the sort order (`reference_num` is the deterministic
/// part; `name` rides along for display).
type GroupKey = (String, String);
type EpicMap = BTreeMap<GroupKey, Vec<FeatureLine>>;
type ReleaseMap = BTreeMap<GroupKey, EpicMap>;

fn group(features: &[Feature]) -> GroupedBacklog {
    let mut by_release: ReleaseMap = BTreeMap::new();

    for f in features {
        let (release_ref, release_name) = match &f.release {
            Some(r) if !r.reference_num.is_empty() => (r.reference_num.clone(), r.name.clone()),
            _ => (NONE_RELEASE_REF.to_string(), String::new()),
        };
        let (epic_ref, epic_name) = match &f.epic {
            Some(e) if !e.reference_num.is_empty() => (e.reference_num.clone(), e.name.clone()),
            _ => (NONE_EPIC_REF.to_string(), String::new()),
        };
        let line = FeatureLine {
            reference_num: f.reference_num.clone(),
            name: f.name.clone(),
            status: f
                .workflow_status
                .as_ref()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "—".into()),
            complete: f
                .workflow_status
                .as_ref()
                .map(|s| s.complete)
                .unwrap_or(false),
            assignee: f
                .assigned_to_user
                .as_ref()
                .and_then(|u| u.email.clone().or_else(|| Some(u.name.clone()))),
        };
        by_release
            .entry((release_ref, release_name))
            .or_default()
            .entry((epic_ref, epic_name))
            .or_default()
            .push(line);
    }

    let releases = by_release
        .into_iter()
        .map(|((release_ref, release_name), epic_map)| {
            let epics = epic_map
                .into_iter()
                .map(|((epic_ref, epic_name), features)| EpicGroup {
                    epic_ref,
                    epic_name,
                    features,
                })
                .collect();
            ReleaseGroup {
                release_ref,
                release_name,
                epics,
            }
        })
        .collect();

    GroupedBacklog { releases }
}

fn render_table(b: &GroupedBacklog) {
    if b.releases.is_empty() {
        println!("(no features match)");
        return;
    }
    for (i, r) in b.releases.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let header = if r.release_name.is_empty() {
            format!("Release: {}", r.release_ref)
        } else {
            format!("Release: {} ({})", r.release_ref, r.release_name)
        };
        println!("{header}");
        for e in &r.epics {
            let epic_header = if e.epic_name.is_empty() {
                format!("  Epic: {}", e.epic_ref)
            } else {
                format!("  Epic: {} ({})", e.epic_ref, e.epic_name)
            };
            println!("{epic_header}");
            for f in &e.features {
                let assignee = f
                    .assignee
                    .as_deref()
                    .map(|a| format!("  <{a}>"))
                    .unwrap_or_default();
                println!(
                    "    [{:<14}] {:<10} {}{}",
                    truncate(&f.status, 14),
                    f.reference_num,
                    f.name,
                    assignee
                );
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max - 1).collect::<String>() + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::models::{EntityRef, WorkflowStatus};

    fn feat(
        refnum: &str,
        release: Option<(&str, &str)>,
        epic: Option<(&str, &str)>,
        status: &str,
        complete: bool,
    ) -> Feature {
        Feature {
            id: refnum.into(),
            reference_num: refnum.into(),
            name: format!("Feature {refnum}"),
            workflow_status: Some(WorkflowStatus {
                id: "s".into(),
                name: status.into(),
                complete,
                color: None,
            }),
            assigned_to_user: None,
            release: release.map(|(r, n)| EntityRef {
                id: r.into(),
                reference_num: r.into(),
                name: n.into(),
            }),
            epic: epic.map(|(e, n)| EntityRef {
                id: e.into(),
                reference_num: e.into(),
                name: n.into(),
            }),
            tags: Vec::new(),
            description: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn groups_by_release_then_epic() {
        let features = vec![
            feat(
                "TC-1",
                Some(("TC-R-1", "R1")),
                Some(("TC-E-1", "E1")),
                "In progress",
                false,
            ),
            feat(
                "TC-2",
                Some(("TC-R-1", "R1")),
                Some(("TC-E-1", "E1")),
                "Done",
                true,
            ),
            feat(
                "TC-3",
                Some(("TC-R-1", "R1")),
                Some(("TC-E-2", "E2")),
                "Open",
                false,
            ),
            feat("TC-4", Some(("TC-R-2", "R2")), None, "Open", false),
            feat("TC-5", None, None, "Open", false),
        ];
        let g = group(&features);
        assert_eq!(g.releases.len(), 3);
        assert_eq!(g.releases[0].release_ref, "(no release)");
        assert_eq!(g.releases[1].release_ref, "TC-R-1");
        assert_eq!(g.releases[1].epics.len(), 2);
        assert_eq!(g.releases[1].epics[0].epic_ref, "TC-E-1");
        assert_eq!(g.releases[1].epics[0].features.len(), 2);
        assert_eq!(g.releases[1].epics[1].epic_ref, "TC-E-2");
        assert_eq!(g.releases[2].release_ref, "TC-R-2");
        assert_eq!(g.releases[2].epics[0].epic_ref, "(no epic)");
    }

    #[test]
    fn empty_input_renders_no_match() {
        let features: Vec<Feature> = Vec::new();
        let g = group(&features);
        assert!(g.releases.is_empty());
    }
}
