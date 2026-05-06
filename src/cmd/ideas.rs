use anyhow::Result;

use crate::client::AhaClient;
use crate::output::{render_list, render_one, OutputFormat};

use super::{status_label, IdeaRow};

pub async fn list(client: &AhaClient, product: Option<&str>, format: OutputFormat) -> Result<()> {
    let ideas = client.list_ideas(product).await?;
    let rows: Vec<IdeaRow> = ideas.iter().map(IdeaRow::from).collect();
    render_list(format, &rows, &ideas)
}

pub async fn show(client: &AhaClient, id: &str, format: OutputFormat) -> Result<()> {
    let i = client.get_idea(id).await?;
    let kv = vec![
        ("id", i.id.clone()),
        ("reference_num", i.reference_num.clone()),
        ("name", i.name.clone()),
        ("status", status_label(&i.workflow_status)),
    ];
    render_one(format, &kv, &i)
}
