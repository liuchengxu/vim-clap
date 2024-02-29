use crate::stdio_server::provider::hooks::PreviewTarget;
use crate::stdio_server::provider::{ClapProvider, Context, ProviderResult as Result};
use crate::types::Goto;
use maple_lsp::lsp;
use matcher::MatchScope;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use printer::Printer;
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
}

impl LocationItem {
    fn from_location(location: &lsp::Location, project_root: &str) -> Option<Self> {
        let file_path = location
            .uri
            .to_file_path()
            .ok()?
            .to_string_lossy()
            .to_string();
        let start_line = location.range.start.line;
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
                        LocationItem::from_location(location, root)
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
        }
    }

    fn process_query(&mut self, query: String, ctx: &Context) -> Result<()> {
        let matcher = ctx.matcher_builder().build(Query::from(&query));

        let items = match &self.source_items {
            SourceItems::Document((_uri, ref items)) => items,
            SourceItems::Workspace(ref items) => items,
            SourceItems::Locations((_goto, ref items)) => items,
            SourceItems::Empty => {
                return Ok(());
            }
        };

        let processed = items.len();

        let mut ranked = filter::par_filter_items(items, &matcher);

        let matched = ranked.len();

        // Only display the top 200 items.
        ranked.truncate(200);

        let mut display_lines = self.printer.to_display_lines(ranked);

        // The indices are empty on the empty query.
        display_lines.indices.retain(|i| !i.is_empty());

        let update_info = printer::PickerUpdateInfo {
            matched,
            processed,
            display_lines,
            ..Default::default()
        };

        ctx.vim.exec("clap#picker#update", update_info)?;

        Ok(())
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
        let preview_target = match &self.source_items {
            SourceItems::Document((uri, _)) => {
                let curline = ctx.vim.display_getcurline().await?;
                let Some(line_number) = curline
                    .split_whitespace()
                    .last()
                    .and_then(|n| n.parse::<usize>().ok())
                else {
                    return Ok(());
                };

                Some(PreviewTarget::LineInFile {
                    path: PathBuf::from(uri.path()),
                    line_number,
                })
            }
            SourceItems::Workspace(_) => {
                let curline = ctx.vim.display_getcurline().await?;
                let Some(path_and_lnum) = curline.split_whitespace().last() else {
                    return Ok(());
                };

                path_and_lnum.split_once(':').and_then(|(path, lnum)| {
                    Some(PreviewTarget::LineInFile {
                        path: path.into(),
                        line_number: lnum.parse::<usize>().ok()?,
                    })
                })
            }
            SourceItems::Locations(_) => {
                let curline = ctx.vim.display_getcurline().await?;
                let Some((fpath, lnum, _col, _cache_line)) =
                    pattern::extract_grep_position(&curline)
                else {
                    return Ok(());
                };

                Some(PreviewTarget::LineInFile {
                    path: fpath.into(),
                    line_number: lnum + 1,
                })
            }
            SourceItems::Empty => return Ok(()),
        };
        ctx.update_preview(preview_target).await
    }

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        ctx.signify_terminated(session_id);
        set_lsp_source(LspSource::Empty);
    }
}
