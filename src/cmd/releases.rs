use anyhow::Result;

use crate::client::AhaClient;
use crate::output::{render_list, render_one, OutputFormat};

use super::ReleaseRow;

pub async fn list(client: &AhaClient, product: Option<&str>, format: OutputFormat) -> Result<()> {
    let releases = client.list_releases(product).await?;
    let rows: Vec<ReleaseRow> = releases.iter().map(ReleaseRow::from).collect();
    render_list(format, &rows, &releases)
}

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let r = client.get_release(id).await?;
    let kv = vec![
        ("id", r.id.clone()),
        ("reference_num", r.reference_num.clone()),
        ("name", r.name.clone()),
        (
            "release_date",
            r.release_date
                .map(|d| d.to_string())
                .unwrap_or_else(|| "—".into()),
        ),
        ("released", r.released.to_string()),
        ("parking_lot", r.parking_lot.to_string()),
        (
            "product_id",
            r.product_id.clone().unwrap_or_else(|| "—".into()),
        ),
    ];
    render_one(format, &kv, &r)
}
