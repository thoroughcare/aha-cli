use anyhow::Result;

use crate::client::AhaClient;
use crate::output::{render_one, OutputFormat};

use super::status_label;

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let r = client.get_requirement(id).await?;
    let kv = vec![
        ("id", r.id.clone()),
        ("reference_num", r.reference_num.clone()),
        ("name", r.name.clone()),
        ("status", status_label(&r.workflow_status)),
    ];
    render_one(format, &kv, &r)
}
