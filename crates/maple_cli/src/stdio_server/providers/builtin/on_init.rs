use std::sync::Arc;

use anyhow::Result;

use crate::stdio_server::session::{Scale, SessionContext};

pub async fn on_create(context: Arc<SessionContext>) -> Result<Scale> {
    if context.provider_id.as_str() == "blines" {
        let total = crate::utils::count_lines(std::fs::File::open(&context.start_buffer_path)?)?;

        let scale = if total > 500_000 {
            Scale::Large(total)
        } else {
            Scale::Small(total)
        };

        return Ok(scale);
    }

    Ok(Scale::Indefinite)
}
