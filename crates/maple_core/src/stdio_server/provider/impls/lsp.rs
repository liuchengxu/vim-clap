use crate::stdio_server::provider::hooks::PreviewTarget;
use crate::stdio_server::provider::{
    ClapProvider, Context, ProviderError, ProviderResult as Result,
};
use crate::types::Goto;
use maple_lsp::lsp;
use matcher::MatchScope;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use printer::{DisplayLines, Printer};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use types::{ClapItem, FuzzyText, Query};

#[derive(Debug)]
pub enum LspSource {
    DocumentSymbols((lsp::Url, Vec<lsp::SymbolInformation>)),
    WorkspaceSymbols(Vec<lsp::SymbolInformation>),
    Locations((Goto, Vec<lsp::Location>)),
    Empty,
}

static LSP_SOURCE: Lazy<Arc<Mutex<LspSource>>> =
    Lazy::new(|| Arc::new(Mutex::new(LspSource::Empty)));

pub fn set_lsp_source(new: LspSource) {
    let mut source = LSP_SOURCE.lock();
    *source = new;
}

const SYMBOL_KINDS: &[&str] = &[
    "Unknown",
    "File",
    "Module",
    "Namespace",
    "Package",
    "Class",
    "Method",
    "Property",
    "Field",
    "Constructor",
    "Enum",
    "Interface",
    "Function",
    "Variable",
    "Constant",
    "String",
    "Number",
    "Boolean",
    "Array",
    "Object",
    "Key",
    "Null",
    "EnumMember",
    "Struct",
    "Event",
    "Operator",
    "TypeParameter",
];

fn to_kind_str(kind: lsp::SymbolKind) -> &'static str {
    match kind {
        lsp::SymbolKind::FILE => SYMBOL_KINDS[1],
        lsp::SymbolKind::MODULE => SYMBOL_KINDS[2],
        lsp::SymbolKind::NAMESPACE => SYMBOL_KINDS[3],
        lsp::SymbolKind::PACKAGE => SYMBOL_KINDS[4],
        lsp::SymbolKind::CLASS => SYMBOL_KINDS[5],
        lsp::SymbolKind::METHOD => SYMBOL_KINDS[6],
        lsp::SymbolKind::PROPERTY => SYMBOL_KINDS[7],
        lsp::SymbolKind::FIELD => SYMBOL_KINDS[8],
        lsp::SymbolKind::CONSTRUCTOR => SYMBOL_KINDS[9],
        lsp::SymbolKind::ENUM => SYMBOL_KINDS[10],
        lsp::SymbolKind::INTERFACE => SYMBOL_KINDS[11],
        lsp::SymbolKind::FUNCTION => SYMBOL_KINDS[12],
        lsp::SymbolKind::VARIABLE => SYMBOL_KINDS[13],
        lsp::SymbolKind::CONSTANT => SYMBOL_KINDS[14],
        lsp::SymbolKind::STRING => SYMBOL_KINDS[15],
        lsp::SymbolKind::NUMBER => SYMBOL_KINDS[16],
        lsp::SymbolKind::BOOLEAN => SYMBOL_KINDS[17],
        lsp::SymbolKind::ARRAY => SYMBOL_KINDS[18],
        lsp::SymbolKind::OBJECT => SYMBOL_KINDS[19],
        lsp::SymbolKind::KEY => SYMBOL_KINDS[20],
        lsp::SymbolKind::NULL => SYMBOL_KINDS[21],
        lsp::SymbolKind::ENUM_MEMBER => SYMBOL_KINDS[22],
        lsp::SymbolKind::STRUCT => SYMBOL_KINDS[23],
        lsp::SymbolKind::EVENT => SYMBOL_KINDS[24],
        lsp::SymbolKind::OPERATOR => SYMBOL_KINDS[25],
        lsp::SymbolKind::TYPE_PARAMETER => SYMBOL_KINDS[26],
        _ => SYMBOL_KINDS[0],
    }
}

#[derive(Debug)]
pub struct LocationItem {
    pub file_path: String,
    pub output_text: String,
    pub location: lsp::Location,
}

impl LocationItem {
    fn from_location(location: lsp::Location, project_root: &str) -> Option<Self> {
        let file_path = location
            .uri
            .to_file_path()
            .ok()?
            .to_string_lossy()
            .to_string();
        // location is 0-based.
        let start_line = location.range.start.line + 1;
        let start_character = location.range.start.character;
        let line = utils::read_line_at(&file_path, start_line as usize)
            .ok()
            .flatten()?;

        let path = file_path
            .strip_prefix(project_root)
            .unwrap_or(file_path.as_str());
        let path = path.strip_prefix(std::path::MAIN_SEPARATOR).unwrap_or(path);
        let output_text = format!("{path}:{start_line}:{start_character}:{line}");

        Some(Self {
            file_path,
            output_text,
            location,
        })
    }
}

impl ClapItem for LocationItem {
    fn raw_text(&self) -> &str {
        &self.output_text
    }

    fn output_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.output_text)
    }

    fn icon(&self, _icon: icon::Icon) -> Option<icon::IconType> {
        Some(icon::file_icon(&self.file_path))
    }
}

#[derive(Debug)]
pub struct DocumentItem {
    pub name: String,
    pub kind: &'static str,
    pub location: lsp::Location,
    pub container_name: Option<String>,
    pub output_text: String,
}

impl DocumentItem {
    fn new(symbol: &lsp::SymbolInformation, name_width: usize) -> Self {
        let kind = to_kind_str(symbol.kind);
        // Convert 0-based to 1-based.
        let line_number = symbol.location.range.start.line + 1;
        let decorated_kind = format!("[{kind}]");
        let output_text = format!(
            "{name:<name_width$} {decorated_kind:<15} {line_number}",
            name = symbol.name
        );

        Self {
            name: symbol.name.to_owned(),
            kind,
            location: symbol.location.clone(),
            container_name: symbol.container_name.clone(),
            output_text,
        }
    }
}

impl ClapItem for DocumentItem {
    fn raw_text(&self) -> &str {
        &self.output_text
    }

    fn fuzzy_text(&self, _match_scope: MatchScope) -> Option<FuzzyText> {
        Some(FuzzyText::new(&self.name, 0))
    }

    fn output_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.output_text)
    }

    fn icon(&self, _icon: icon::Icon) -> Option<icon::IconType> {
        Some(icon::tags_kind_icon(&self.kind.to_lowercase()))
    }
}

#[derive(Debug)]
pub struct WorkspaceItem {
    pub name: String,
    pub path: String,
    pub kind: &'static str,
    pub location: lsp::Location,
    pub container_name: Option<String>,
    pub output_text: String,
}

impl WorkspaceItem {
    fn new(symbol: &lsp::SymbolInformation, name_width: usize) -> Self {
        let kind = to_kind_str(symbol.kind);
        let path = symbol.location.uri.path().to_owned();
        // Convert 0-based to 1-based.
        let line_number = symbol.location.range.start.line + 1;
        let decorated_kind = format!("[{kind}]");
        let output_text = format!(
            "{name:<name_width$} {decorated_kind:<15} {path}:{line_number}",
            name = symbol.name,
        );

        Self {
            name: symbol.name.to_owned(),
            path,
            kind,
            location: symbol.location.clone(),
            container_name: symbol.container_name.clone(),
            output_text,
        }
    }
}

impl ClapItem for WorkspaceItem {
    fn raw_text(&self) -> &str {
        &self.output_text
    }

    fn fuzzy_text(&self, _match_scope: MatchScope) -> Option<FuzzyText> {
        Some(FuzzyText::new(&self.name, 0))
    }

    fn output_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.output_text)
    }

    fn icon(&self, _icon: icon::Icon) -> Option<icon::IconType> {
        Some(icon::tags_kind_icon(&self.kind.to_lowercase()))
    }
}

#[derive(Debug)]
enum SourceItems {
    Document((lsp::Url, Vec<Arc<dyn ClapItem>>)),
    Workspace(Vec<Arc<dyn ClapItem>>),
    Locations((Goto, Vec<Arc<dyn ClapItem>>)),
    Empty,
}

#[derive(Debug)]
pub struct LspProvider {
    printer: Printer,
    source_items: SourceItems,
    current_items: Vec<Arc<dyn ClapItem>>,
    current_display_lines: DisplayLines,
}

impl LspProvider {
    pub fn new(ctx: &Context) -> Self {
        let printer = Printer::new(ctx.env.display_winwidth, ctx.env.icon);

        // NOTE: lsp source must be initialized before invoking this provider.
        let source_items = match *LSP_SOURCE.lock() {
            LspSource::DocumentSymbols((ref uri, ref symbols)) => {
                let max_length = symbols.iter().map(|s| s.name.len()).max().unwrap_or(20);
                let items = symbols
                    .iter()
                    .map(|symbol| {
                        Arc::new(DocumentItem::new(symbol, max_length)) as Arc<dyn ClapItem>
                    })
                    .collect::<Vec<_>>();

                SourceItems::Document((uri.clone(), items))
            }
            LspSource::WorkspaceSymbols(ref symbols) => {
                let max_len = symbols.iter().map(|s| s.name.len()).max().unwrap_or(15);
                let items = symbols
                    .iter()
                    .map(|symbol| {
                        Arc::new(WorkspaceItem::new(symbol, max_len)) as Arc<dyn ClapItem>
                    })
                    .collect::<Vec<_>>();

                SourceItems::Workspace(items)
            }
            LspSource::Locations((goto, ref locations)) => {
                let root = ctx.cwd.as_str();
                let items = locations
                    .iter()
                    .filter_map(|location| {
                        LocationItem::from_location(location.clone(), root)
                            .map(|item| Arc::new(item) as Arc<dyn ClapItem>)
                    })
                    .collect::<Vec<_>>();

                SourceItems::Locations((goto, items))
            }
            LspSource::Empty => SourceItems::Empty,
        };

        Self {
            printer,
            source_items,
            current_items: Vec::new(),
            current_display_lines: Default::default(),
        }
    }

    fn fetch_location_at(&self, line_number: usize) -> Option<FileLocation> {
        match &self.source_items {
            SourceItems::Document((uri, _)) => {
                let doc_item = self
                    .current_items
                    .get(line_number - 1)
                    .and_then(|item| item.as_any().downcast_ref::<DocumentItem>())?;

                Some(FileLocation {
                    path: uri.path().to_string(),
                    row: doc_item.location.range.start.line as usize + 1,
                    column: doc_item.location.range.start.character as usize + 1,
                })
            }
            SourceItems::Workspace(_) => {
                let workspace_item = self
                    .current_items
                    .get(line_number - 1)
                    .and_then(|item| item.as_any().downcast_ref::<WorkspaceItem>())?;

                Some(FileLocation {
                    path: workspace_item.path.clone(),
                    row: workspace_item.location.range.start.line as usize + 1,
                    column: workspace_item.location.range.start.character as usize + 1,
                })
            }
            SourceItems::Locations(_) => {
                let location_item = self
                    .current_items
                    .get(line_number - 1)
                    .and_then(|item| item.as_any().downcast_ref::<LocationItem>())?;

                Some(FileLocation {
                    path: location_item.file_path.clone(),
                    row: location_item.location.range.start.line as usize + 1,
                    column: location_item.location.range.start.character as usize + 1,
                })
            }
            SourceItems::Empty => None,
        }
    }

    fn process_query(&mut self, query: String, ctx: &Context) -> Result<()> {
        let items = match &self.source_items {
            SourceItems::Document((_uri, ref items)) => items,
            SourceItems::Workspace(ref items) => items,
            SourceItems::Locations((_goto, ref items)) => items,
            SourceItems::Empty => {
                return Ok(());
            }
        };

        let matcher = ctx.matcher_builder().build(Query::from(&query));

        let mut ranked = filter::par_filter_items(items, &matcher);

        let matched = ranked.len();

        // Only display the top 200 items.
        ranked.truncate(200);

        self.current_items = ranked.iter().map(|r| r.item.clone()).collect();
        let display_lines = self.printer.to_display_lines(ranked);

        let update_info = printer::PickerUpdateInfo {
            matched,
            processed: items.len(),
            display_lines,
            ..Default::default()
        };

        ctx.vim.exec("clap#picker#update", &update_info)?;

        self.current_display_lines = update_info.display_lines;

        Ok(())
    }
}

struct FileLocation {
    path: String,
    // 1-based
    row: usize,
    // 1-based
    column: usize,
}

impl FileLocation {
    fn into_preview_target(self) -> PreviewTarget {
        PreviewTarget::LineInFile {
            path: PathBuf::from(self.path),
            line_number: self.row,
        }
    }
}

#[async_trait::async_trait]
impl ClapProvider for LspProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        self.process_query("".to_owned(), ctx)
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;
        self.process_query(query, ctx)
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        if !ctx.env.preview_enabled {
            return Ok(());
        }
        ctx.preview_manager.reset_scroll();

        let line_number = ctx.vim.display_getcurlnum().await?;
        let loc = self
            .fetch_location_at(line_number)
            .ok_or(ProviderError::PreviewItemNotFound { line_number })?;
        ctx.update_preview(Some(loc.into_preview_target())).await
    }

    async fn remote_sink(&mut self, ctx: &mut Context, line_numbers: Vec<usize>) -> Result<()> {
        if line_numbers.len() == 1 {
            let line_number = line_numbers[0];
            let loc = self
                .fetch_location_at(line_number)
                .ok_or(ProviderError::PreviewItemNotFound { line_number })?;
            ctx.vim.exec(
                "clap#plugin#lsp#jump_to",
                serde_json::json!({
                  "path": loc.path,
                  "row": loc.row,
                  "column": loc.column
                }),
            )?;
        } else {
            let locs = line_numbers
                .into_iter()
                .filter_map(|line_number| self.fetch_location_at(line_number))
                .filter_map(|loc| {
                    let text = utils::read_line_at(&loc.path, loc.row).ok().flatten()?;
                    Some(serde_json::json!({
                      "filename": loc.path,
                      "lnum": loc.row,
                      "col": loc.column,
                      "text": text
                    }))
                })
                .collect::<Vec<_>>();
            ctx.vim.exec("clap#sink#open_quickfix", [locs])?;
        }
        Ok(())
    }

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        ctx.signify_terminated(session_id);
        set_lsp_source(LspSource::Empty);
    }
}
