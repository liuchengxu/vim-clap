mod blines;
mod dumb_jump;
pub mod filer;
mod files;
mod generic_provider;
mod grep;
mod igrep;
pub mod lsp;
mod recent_files;
mod tagfiles;

use crate::stdio_server::provider::{ClapProvider, Context, ProviderResult};

pub async fn create_provider(ctx: &Context) -> ProviderResult<Box<dyn ClapProvider>> {
    let provider: Box<dyn ClapProvider> = match ctx.env.provider_id.as_str() {
        "blines" => Box::new(blines::BlinesProvider::new(ctx).await?),
        "dumb_jump" => Box::new(dumb_jump::DumbJumpProvider::new(ctx).await?),
        "filer" => Box::new(filer::FilerProvider::new(ctx).await?),
        "files" => Box::new(files::FilesProvider::new(ctx).await?),
        "grep" => Box::new(grep::GrepProvider::new(ctx).await?),
        "igrep" => Box::new(igrep::IgrepProvider::new(ctx).await?),
        "recent_files" => Box::new(recent_files::RecentFilesProvider::new(ctx).await?),
        "tagfiles" => Box::new(tagfiles::TagfilesProvider::new(ctx).await?),
        "lsp" => Box::new(lsp::LspProvider::new(ctx)),
        _ => Box::new(generic_provider::GenericProvider::new(ctx).await?),
    };
    Ok(provider)
}
