use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::stdio_server::rpc::Params;
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
    pub fn parse_filetypedetect(self) -> Value {
        let msg = self;
        let output = msg.get_string_unsafe("autocmd_filetypedetect");
        let ext_map = crate::stdio_server::vim::initialize_syntax_map(&output);
        json!({ "method": "clap#ext#set", "ext_map": ext_map })
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

        let fpath = crate::utils::expand_tilde(fpath);

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

fn parse_quickfix_entry(line: &str) -> Result<(&str, usize)> {
    let mut splitted = line.split('|');
    let fpath = splitted
        .next()
        .ok_or_else(|| anyhow!("Can not find fpath in {}", line))?;

    let mut it = splitted
        .next()
        .ok_or_else(|| anyhow!("Can not find lnum and column info in {}", line))?
        .split("col");

    let lnum = it
        .next()
        .ok_or_else(|| anyhow!("Can not find lnum in {}", line))?
        .trim()
        .parse::<usize>()?;

    Ok((fpath, lnum))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quickdix_display_line_works() {
        let line = "test/bench/python/test_fuzzy_filter.vim|0 col 0| Modified 2月,13 2021 10:58:27 rw-rw-r--";
        assert_eq!(
            parse_quickfix_entry(line).unwrap(),
            ("test/bench/python/test_fuzzy_filter.vim".into(), 0usize)
        );
    }
}
