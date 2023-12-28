use crate::stdio_server::provider::{ClapProvider, Context, ProviderResult as Result};
use maple_lsp::lsp;
use matcher::MatchScope;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use printer::Printer;
use serde_json::json;
use std::borrow::Cow;
use std::sync::Arc;
use types::{ClapItem, FuzzyText, Query};

#[derive(Debug)]
pub enum LspSource {
    DocumentSymbols(Vec<lsp::SymbolInformation>),
    WorkspaceSymbols(Vec<lsp::SymbolInformation>),
    Empty,
}

static LSP_SOURCE: Lazy<Arc<Mutex<LspSource>>> =
    Lazy::new(|| Arc::new(Mutex::new(LspSource::Empty)));

pub fn set_lsp_source(new: LspSource) {
    let mut source = LSP_SOURCE.lock();
    *source = new;
}

#[derive(Debug)]
pub struct LspProvider {
    printer: Printer,
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
pub struct DocumentItem {
    pub name: String,
    pub kind: &'static str,
    pub location: lsp::Location,
    pub container_name: Option<String>,
    pub output_text: String,
}

impl DocumentItem {
    fn new(symbol: &lsp::SymbolInformation) -> Self {
        let kind = to_kind_str(symbol.kind);
        let line = symbol.location.range.start.line;
        let output_text = format!(
            "{name:<name_width$} [{kind}] {line}",
            name = symbol.name,
            name_width = 10,
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
    fn new(symbol: &lsp::SymbolInformation) -> Self {
        let kind = to_kind_str(symbol.kind);
        let path = symbol.location.uri.path().to_owned();
        let line = symbol.location.range.start.line;
        let output_text = format!(
            "{name:<name_width$} [{kind}] {path}:{line}",
            name = symbol.name,
            name_width = 10,
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

impl LspProvider {
    pub fn new(ctx: &Context) -> Self {
        let icon = if ctx.env.icon.enabled() {
            icon::Icon::Enabled(icon::IconKind::File)
        } else {
            icon::Icon::Null
        };
        let printer = Printer::new(ctx.env.display_winwidth, icon);

        Self { printer }
    }

    fn process_query(&mut self, query: String, ctx: &Context) -> Result<()> {
        let matcher = ctx.matcher_builder().build(Query::from(&query));

        let source_items = match *LSP_SOURCE.lock() {
            LspSource::DocumentSymbols(ref symbols) => symbols
                .iter()
                .map(|symbol| Arc::new(DocumentItem::new(symbol)) as Arc<dyn ClapItem>)
                .collect::<Vec<_>>(),
            LspSource::WorkspaceSymbols(ref symbols) => symbols
                .iter()
                .map(|symbol| Arc::new(WorkspaceItem::new(symbol)) as Arc<dyn ClapItem>)
                .collect::<Vec<_>>(),
            LspSource::Empty => {
                return Ok(());
            }
        };

        let ranked = filter::par_filter(source_items, &matcher);

        let printer::DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = self
            .printer
            .to_display_lines(ranked.iter().take(200).cloned().collect());

        // The indices are empty on the empty query.
        let indices = indices
            .into_iter()
            .filter(|i| !i.is_empty())
            .collect::<Vec<_>>();

        let mut value = json!({
            "lines": lines,
            "indices": indices,
            "matched": ranked.len(),
            "processed": ranked.len(),
            "icon_added": icon_added,
            "preview": Option::<serde_json::Value>::None,
        });

        if !truncated_map.is_empty() {
            value
                .as_object_mut()
                .expect("Value is constructed as an Object")
                .insert("truncated_map".into(), json!(truncated_map));
        }

        ctx.vim.exec("clap#state#update_picker", value)?;

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

    fn on_terminate(&mut self, ctx: &mut Context, session_id: u64) {
        ctx.signify_terminated(session_id);
        set_lsp_source(LspSource::Empty);
    }
}
