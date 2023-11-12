use crate::stdio_server::input::ActionRequest;
use crate::stdio_server::plugin::{ClapPlugin, PluginError};
use crate::stdio_server::vim::Vim;
use clipboard::{ClipboardContext, ClipboardProvider};

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "system", actions = ["__note_recent_files", "__copy-to-clipboard", "open-config", "list-plugins"])]
pub struct System {
    vim: Vim,
}

impl System {
    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }
}

#[async_trait::async_trait]
impl ClapPlugin for System {
    async fn handle_action(&mut self, action: ActionRequest) -> Result<(), PluginError> {
        let ActionRequest { method, params } = action;

        match self.parse_action(method)? {
            SystemAction::__NoteRecentFiles => {
                let bufnr: Vec<usize> = params.parse()?;
                let bufnr = bufnr
                    .first()
                    .ok_or(PluginError::MissingBufferNumber("note_recent_files"))?;
                let file_path: String = self.vim.expand(format!("#{bufnr}:p")).await?;
                crate::stdio_server::handler::messages::note_recent_file(file_path);
            }
            SystemAction::__CopyToClipboard => {
                let content: Vec<String> = params.parse()?;

                let mut ctx: ClipboardContext =
                    ClipboardProvider::new().map_err(PluginError::Clipboard)?;
                match ctx.set_contents(content.into_iter().next().unwrap()) {
                    Ok(()) => {
                        self.vim.echo_info("copied to clipboard successfully")?;
                    }
                    Err(e) => {
                        self.vim
                            .echo_warn(format!("failed to copy to clipboard: {e:?}"))?;
                    }
                }
            }
            SystemAction::OpenConfig => {
                let config_file = crate::config::config_file();
                self.vim
                    .exec("execute", format!("edit {}", config_file.display()))?;
            }
            SystemAction::ListPlugins => {
                unreachable!("action list-plugins has been handled upper level")
            }
        }

        Ok(())
    }
}
