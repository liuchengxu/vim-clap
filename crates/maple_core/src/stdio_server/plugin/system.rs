use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::input::PluginAction;
use crate::stdio_server::plugin::{ClapPlugin, PluginError, PluginResult};
use crate::stdio_server::vim::Vim;
use copypasta::{ClipboardContext, ClipboardProvider};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "system",
  actions = [
    "__noteRecentFiles",
    "__copyToClipboard",
    "__configureVimWhichKey",
    "__didYouMean",
    "openConfig",
    "listPlugins",
  ]
)]
pub struct System {
    vim: Vim,
}

impl System {
    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }

    pub fn is_list_plugins(plugin_id: &str, action: &PluginAction) -> bool {
        plugin_id == "system" && action.method == "listPlugins"
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

    let Ok(path) = std::fs::canonicalize(&file_path) else {
        return;
    };

    if !path.exists() || !path.is_file() {
        return;
    }

    let mut recent_files = RECENT_FILES_IN_MEMORY.write();
    recent_files.upsert(file_path);
}

// https://github.com/clap-rs/clap/blob/c0a1814d3c1d93c18faaedc95fd251846e47f4fe/clap_builder/src/parser/features/suggestions.rs#L11C1-L26C2
fn did_you_mean<T, I>(v: &str, possible_values: I) -> Vec<String>
where
    T: AsRef<str>,
    I: IntoIterator<Item = T>,
{
    let mut candidates: Vec<(f64, String)> = possible_values
        .into_iter()
        // GH #4660: using `jaro` because `jaro_winkler` implementation in `strsim-rs` is wrong
        // causing strings with common prefix >=10 to be considered perfectly similar
        .map(|pv| (strsim::jaro(v, pv.as_ref()), pv.as_ref().to_owned()))
        // Confidence of 0.7 so that bar -> baz is suggested
        .filter(|(confidence, _)| *confidence > 0.7)
        .collect();
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates.into_iter().map(|(_, pv)| pv).collect()
}

#[async_trait::async_trait]
impl ClapPlugin for System {
    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        let PluginAction { method, params } = action;

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
                let content = content.into_iter().next().ok_or_else(|| {
                    PluginError::Other("missing content in __copy-to-clipboard".to_string())
                })?;

                let mut ctx = ClipboardContext::new().map_err(PluginError::Clipboard)?;
                match ctx.set_contents(content) {
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
            SystemAction::__DidYouMean => {
                let args: Vec<String> = params.parse()?;
                let input = &args[0];
                let all_providers: Vec<String> = self
                    .vim
                    .bare_call("clap#provider#providers#get_all_provider_ids")
                    .await?;
                let mut candidates = did_you_mean(input, all_providers);
                if let Some(suggestion) = candidates.pop() {
                    self.vim.echo_message(format!(
                        "provider `{input}` is not found, did you mean `:Clap {suggestion}`?"
                    ))?;
                } else {
                    self.vim
                        .echo_message(format!("provider {input} not found"))?;
                }
            }
            SystemAction::OpenConfig => {
                let config_file = maple_config::config_file();
                self.vim
                    .exec("execute", format!("edit {}", config_file.display()))?;
            }
            SystemAction::ListPlugins => {
                self.vim
                    .echo_warn("action listPlugins should have been handled upper level, please report this as an error")?;
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
