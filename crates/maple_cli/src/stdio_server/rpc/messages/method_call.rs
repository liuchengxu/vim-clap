use std::collections::HashMap;

use anyhow::{anyhow, Result};
use jsonrpc_core::Params;
use rayon::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::stdio_server::types::ProviderId;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MethodCall {
    pub id: u64,
    pub method: String,
    pub params: Params,
    pub session_id: u64,
}

impl MethodCall {
    pub fn parse<T: DeserializeOwned>(self) -> Result<T> {
        self.params.parse().map_err(Into::into)
    }

    pub fn parse_unsafe<T: DeserializeOwned>(self) -> T {
        self.parse()
            .unwrap_or_else(|e| panic!("Couldn't deserialize params: {:?}", e))
    }

    pub fn get_query(&self) -> String {
        self.get_string_unsafe("query")
    }

    pub fn get_cwd(&self) -> String {
        self.get_string_unsafe("cwd")
    }

    /// Get the current line of display window without the leading icon.
    pub fn get_curline(&self, provider_id: &ProviderId) -> Result<String> {
        let display_curline = self.get_string("curline")?;

        let curline = if let Ok(enable_icon) = self.get_bool("enable_icon") {
            if enable_icon {
                display_curline.chars().skip(2).collect()
            } else {
                display_curline
            }
        } else if provider_id.should_skip_leading_icon() {
            display_curline.chars().skip(2).collect()
        } else {
            display_curline
        };

        Ok(curline)
    }

    fn map_params(&self) -> Result<&serde_json::Map<String, Value>> {
        match &self.params {
            Params::None => Err(anyhow!("None params unsupported")),
            Params::Array(_) => Err(anyhow!("Array params unsupported")),
            Params::Map(map) => Ok(map),
        }
    }

    #[allow(unused)]
    pub fn get_u64(&self, key: &str) -> Result<u64> {
        self.map_params()?
            .get(key)
            .and_then(|x| x.as_u64())
            .ok_or_else(|| anyhow!("Missing {} in msg.params", key))
    }

    pub fn get_str(&self, key: &str) -> Result<&str> {
        self.map_params()?
            .get(key)
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow!("Missing {} in msg.params", key))
    }

    pub fn get_string(&self, key: &str) -> Result<String> {
        self.get_str(key).map(Into::into)
    }

    pub fn get_string_unsafe(&self, key: &str) -> String {
        self.get_string(key)
            .unwrap_or_else(|e| panic!("Get String error: {:?}", e))
    }

    pub fn get_bool(&self, key: &str) -> Result<bool> {
        self.map_params()?
            .get(key)
            .and_then(|x| x.as_bool())
            .ok_or_else(|| anyhow!("Missing {} in msg.params", key))
    }
}

impl MethodCall {
    pub async fn handle(self) -> anyhow::Result<Value> {
        use crate::stdio_server::providers::dumb_jump::DumbJumpSession;
        use crate::stdio_server::providers::recent_files::RecentFilesSession;
        use crate::stdio_server::session::SessionEvent::*;

        if self.method != "init_ext_map" {
            tracing::debug!(message = ?self, "==> stdio message(in)");
        }

        let value = match self.method.as_str() {
            "init_ext_map" => self.parse_filetypedetect(),
            "preview/file" => self.preview_file().await?,
            // "quickfix" => super::quickfix::preview_quickfix_entry(msg),

            /*
            "dumb_jump/on_init" => manager.new_session::<DumbJumpSession>(msg),
            "dumb_jump/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "dumb_jump/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "recent_files/on_init" => manager.new_session::<RecentFilesSession>(msg),
            "recent_files/on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "recent_files/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "filer" => filer::handle_filer_message(msg),
            "filer/on_init" => manager.new_session::<FilerSession>(msg),
            "filer/on_move" => manager.send(msg.session_id, OnMove(msg)),

            "on_init" => manager.new_session::<BuiltinSession>(msg),
            "on_typed" => manager.send(msg.session_id, OnTyped(msg)),
            "on_move" => manager.send(msg.session_id, OnMove(msg)),
            "exit" => manager.terminate(msg.session_id),
            */
            _ => json!({ "error": format!("unknown method: {}", self.method), "id": self.id }),
        };

        Ok(value)
    }

    pub fn parse_filetypedetect(self) -> Value {
        let msg = self;
        let output = msg.get_string_unsafe("autocmd_filetypedetect");
        let ext_map: HashMap<&str, &str> = output
            .par_split(|x| x == '\n')
            .filter(|s| s.contains("setf"))
            .filter_map(|s| {
                // *.mkiv    setf context
                let items = s.split_whitespace().collect::<Vec<_>>();
                if items.len() != 3 {
                    None
                } else {
                    // (mkiv, context)
                    items[0].split('.').last().map(|ext| (ext, items[2]))
                }
            })
            .chain(
                vec![("h", "c"), ("hpp", "cpp"), ("vimrc", "vim"), ("cc", "cpp")].into_par_iter(),
            )
            .map(|(ext, ft)| (ext, ft))
            .collect();

        json!({"method": "clap#ext#set", "ext_map": ext_map})
    }

    pub async fn preview_file(self) -> Result<Value> {
        let msg_id = self.id;

        #[derive(Deserialize)]
        struct InnerParams {
            fpath: String,
            display_width: usize,
            display_height: usize,
            preview_width: Option<usize>,
            preview_height: Option<usize>,
            preview_direction: String,
        }

        let InnerParams {
            fpath,
            display_width,
            display_height,
            preview_width,
            preview_height,
            preview_direction,
        } = self.params.parse()?;

        let fpath = crate::utils::expand_tilde(fpath)?;

        let (preview_height, preview_width) = if preview_direction.to_uppercase().as_str() == "UD" {
            (preview_height.unwrap_or(display_height), display_width)
        } else {
            (display_height, preview_width.unwrap_or(display_width))
        };

        let (lines, fname) = crate::previewer::preview_file(fpath, preview_height, preview_width)?;

        let value = json!({"id": msg_id, "result": json!({"lines": lines, "fname": fname})});

        Ok(value)
    }

    pub async fn preview_quickfix(self) -> Result<Value> {
        use crate::previewer::{preview_file, preview_file_at};
        use crate::stdio_server::providers::custom::quickfix::parse_quickfix_entry;
        use std::path::PathBuf;

        let msg_id = self.id;

        #[derive(Deserialize)]
        struct InnerParams {
            cwd: String,
            curline: String,
            winwidth: u64,
            winheight: u64,
        }

        let InnerParams {
            cwd,
            curline,
            winwidth,
            winheight,
        } = self.params.parse()?;

        let (p, lnum) = parse_quickfix_entry(curline.as_str())?;

        let mut fpath: PathBuf = cwd.into();
        fpath.push(p);

        let result = if lnum == 0 {
            let size = (winheight + 5) as usize;
            let (lines, _) = preview_file(fpath.as_path(), size, winwidth as usize)?;
            json!({ "event": "on_move", "lines": lines, "fname": fpath })
        } else {
            let size = (winheight / 2) as usize;
            let (lines, hi_lnum) = preview_file_at(fpath.as_path(), size, winwidth as usize, lnum)?;
            json!({ "event": "on_move", "lines": lines, "fname": fpath, "hi_lnum": hi_lnum })
        };

        let value = json!({ "id": msg_id, "provider_id": "quickfix", "result": result });

        Ok(value)
    }
}
