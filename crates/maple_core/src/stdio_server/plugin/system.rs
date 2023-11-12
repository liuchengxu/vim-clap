use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::input::ActionRequest;
use crate::stdio_server::plugin::{ClapPlugin, PluginError, PluginResult};
use crate::stdio_server::vim::Vim;
use copypasta::{ClipboardContext, ClipboardProvider};
use std::collections::HashMap;

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "system", actions = ["__note_recent_files", "__copy-to-clipboard", "__configure-vim-which-key", "open-config", "list-plugins"])]
pub struct System {
    vim: Vim,
}

impl System {
    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }

    async fn configure_vim_which_key_map(
        &self,
        variable_name: &str,
        config_files: &[String],
    ) -> PluginResult<()> {
        let mut final_map = HashMap::new();

        for config_file in config_files {
            final_map.extend(parse_vim_which_key_map(config_file));
        }

        self.vim.set_var(variable_name, final_map)?;

        Ok(())
    }
}

fn parse_vim_which_key_map(config_file: &str) -> HashMap<char, HashMap<char, String>> {
    use once_cell::sync::Lazy;
    use regex::Regex;

    static COMMENT_DOC: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"^\s*\"\"\" (.*?): (.*)"#).unwrap());

    let mut map = HashMap::new();

    if let Ok(lines) = utils::read_lines(config_file) {
        lines.for_each(|line| {
            if let Ok(line) = line {
                if let Some(caps) = COMMENT_DOC.captures(&line) {
                    let keys = caps.get(1).map(|x| x.as_str()).unwrap();
                    let desc = caps.get(2).map(|x| x.as_str()).unwrap();

                    let mut chars = keys.chars();
                    let k1 = chars.next().unwrap();
                    let k2 = chars.next().unwrap();

                    map.entry(k1)
                        .or_insert_with(HashMap::new)
                        .insert(k2, desc.to_string());
                }
            }
        });
    }

    map
}

fn note_recent_file(file_path: String) {
    tracing::debug!(?file_path, "Received a recent file notification");

    let path = std::path::Path::new(&file_path);
    if !path.exists() || !path.is_file() {
        return;
    }

    let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
    recent_files.upsert(file_path);
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
                note_recent_file(file_path);
            }
            SystemAction::__CopyToClipboard => {
                let content: Vec<String> = params.parse()?;

                let mut ctx = ClipboardContext::new().map_err(PluginError::Clipboard)?;
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
            SystemAction::__ConfigureVimWhichKey => {
                let args: Vec<String> = params.parse()?;
                self.configure_vim_which_key_map(&args[0], &args[1..])
                    .await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vim_which_key_map() {
        parse_vim_which_key_map("/home/xlc/.vimrc");
    }
}
