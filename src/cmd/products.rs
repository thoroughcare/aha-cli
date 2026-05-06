use anyhow::Result;

use crate::client::AhaClient;
use crate::output::{render_list, OutputFormat};

use super::ProductRow;

pub async fn list(client: &AhaClient, format: OutputFormat) -> Result<()> {
    let products = client.list_products().await?;
    let rows: Vec<ProductRow> = products.iter().map(ProductRow::from).collect();
    render_list(format, &rows, &products)
}
