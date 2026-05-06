use anyhow::Result;

use crate::client::AhaClient;
use crate::output::{render_list, render_one, OutputFormat};

use super::{status_label, EpicRow};

pub async fn list(
    client: &AhaClient,
    product: Option<&str>,
    release: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let epics = client.list_epics(product, release).await?;
    let rows: Vec<EpicRow> = epics.iter().map(EpicRow::from).collect();
    render_list(format, &rows, &epics)
}

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let e = client.get_epic(id).await?;
    let kv = vec![
        ("id", e.id.clone()),
        ("reference_num", e.reference_num.clone()),
        ("name", e.name.clone()),
        ("status", status_label(&e.workflow_status)),
        (
            "release",
            e.release
                .as_ref()
                .map(|r| format!("{} ({})", r.reference_num, r.name))
                .unwrap_or_else(|| "—".into()),
        ),
    ];
    render_one(format, &kv, &e)
}
