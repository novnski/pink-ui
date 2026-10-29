//! File Tree – Pink's workspace file tree: the Files pane's search toolbar
//! over the project's folders and files.
//!
//! Rows are Pink's 27 px tree rows, drawn by the `pink-file-row` package
//! ([`crate::components::file_row`]): a chevron that turns for folders, the
//! file-type artwork (Symbols, the VS Code theme Pink bundles) and the name,
//! indented 14 px a level inside an inset, rounded selection pill. The
//! selection takes the accent while the tree has focus and a quiet wash when
//! it does not; arrow keys walk, open and close folders like a native
//! sidebar. Folders that list lazily show Pink's Loading…, Empty folder,
//! error and Load more… rows. Hidden and ignored entries stay out until the
//! eye shows them, dimmed. The search field finds files by name (a tree of
//! matches under their folders) or, in text mode, lines inside files.
//!
//! ```ignore
//! FileTree::new("files")
//!     .entry(FileEntry::folder("src").child(FileEntry::file("main.rs")))
//!     .entry(FileEntry::file("Cargo.toml"))
//!     .expanded(["src"])
//!     .on_open(|path, _, _| println!("open {path}"))
//! ```

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Context, Div, DragMoveEvent, ElementId, Entity, FocusHandle,
    HighlightStyle, Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Render, Role,
    ScrollHandle, SharedString, Stateful, StyledText, Subscription, Task, Toggled, Window, div,
    img, point, prelude::*, px, svg,
};

use super::scroll_rail;
use crate::{
    HoverFill,
    components::file_row,
    motion,
    pink::{self, FillTier, Pink, chrome, layout, loaders, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "File Tree";
pub const DESCRIPTION: &str = "A project's folders and files in a quiet sidebar tree: folders that fold, keyboard walking, file-type artwork, and a search field that finds files by name or lines inside them";

const ICON_CHEVRON: &str = "icons/file-tree/alt-arrow-right.svg";
const ICON_CHEVRON_DOWN: &str = "icons/file-tree/alt-arrow-down.svg";
const ICON_SEARCH: &str = "icons/file-tree/magnifer.svg";
const ICON_TEXT_SEARCH: &str = "icons/file-tree/file-search.svg";
const ICON_EYE: &str = "icons/file-tree/eye.svg";
const ICON_EYE_CLOSED: &str = "icons/file-tree/eye-closed.svg";
const ICON_FOLDER: &str = "icons/file-tree/folder.svg";

/// Pink: files/search.rs `SEARCH_RESULT_LIMIT`.
const SEARCH_RESULT_LIMIT: usize = 200;
/// Pink: files/search.rs – the name search waits this long after a keystroke.
const NAME_SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
/// Pink: files/text_search.rs `TEXT_SEARCH_DEBOUNCE`.
const TEXT_SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
/// Pink: files/preview.rs `TREE_SPLIT_DEFAULT`.
const DEFAULT_WIDTH: f32 = 286.0;
/// How long a lazy folder takes to list (Pink waits on the workspace host).
const DEFAULT_LOAD_DELAY: Duration = Duration::from_millis(650);

// Pink: popover.rs `MENU_SCROLLBAR_*` – the floating rail the tree wears,
// drawn by the `pink-scroll-rail` package.
const RAIL_TRACK_INSET: f32 = scroll_rail::TRACK_INSET;
const RAIL_MIN_THUMB: f32 = scroll_rail::MIN_THUMB;
const RAIL_LINGER_MS: f32 = 1400.0;
const RAIL_FADE_MS: f32 = 260.0;

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// What an entry is (Pink: proto `WorkspaceEntryKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Folder,
    File,
    Symlink,
}

impl EntryKind {
    /// Pink: files/model.rs `entry_rank` – folders list first.
    fn rank(self) -> u8 {
        match self {
            EntryKind::Folder => 0,
            EntryKind::File => 1,
            EntryKind::Symlink => 2,
        }
    }
}

/// How a folder lists its entries (Pink: files/model.rs `DirectoryLoadState`).
#[derive(Clone, Debug, PartialEq)]
pub enum FolderLoad {
    /// Already listed.
    Ready,
    /// Listed the first time it opens, behind a Loading… row.
    Lazy,
    /// Listing fails with this message until someone retries.
    Error(SharedString),
    /// Listed this many entries at a time, with a Load more… row.
    Paged(usize),
}

/// One file or folder in the workspace.
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: SharedString,
    pub kind: EntryKind,
    pub children: Vec<FileEntry>,
    /// Hidden or git-ignored: left out until the eye shows it, then dimmed.
    pub ignored: bool,
    /// The file's text, searched by Search text in files.
    pub text: Option<SharedString>,
    pub load: FolderLoad,
}

impl FileEntry {
    fn new(name: impl Into<SharedString>, kind: EntryKind) -> Self {
        Self {
            name: name.into(),
            kind,
            children: Vec::new(),
            ignored: false,
            text: None,
            load: FolderLoad::Ready,
        }
    }

    pub fn file(name: impl Into<SharedString>) -> Self {
        Self::new(name, EntryKind::File)
    }

    pub fn folder(name: impl Into<SharedString>) -> Self {
        Self::new(name, EntryKind::Folder)
    }

    pub fn symlink(name: impl Into<SharedString>) -> Self {
        Self::new(name, EntryKind::Symlink)
    }

    pub fn child(mut self, entry: FileEntry) -> Self {
        self.children.push(entry);
        self
    }

    pub fn children(mut self, entries: impl IntoIterator<Item = FileEntry>) -> Self {
        self.children.extend(entries);
        self
    }

    pub fn ignored(mut self, ignored: bool) -> Self {
        self.ignored = ignored;
        self
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn load(mut self, load: FolderLoad) -> Self {
        self.load = load;
        self
    }

    /// Lists its entries the first time it opens.
    pub fn lazy(self) -> Self {
        self.load(FolderLoad::Lazy)
    }

    /// Fails to list, with `message`, until retried.
    pub fn error(self, message: impl Into<SharedString>) -> Self {
        self.load(FolderLoad::Error(message.into()))
    }

    /// Lists `page` entries at a time.
    pub fn paged(self, page: usize) -> Self {
        self.load(FolderLoad::Paged(page.max(1)))
    }
}

/// What the search field looks for (Pink: files/text_search.rs `FileSearchMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// File and folder names (the default).
    #[default]
    Names,
    /// Lines inside files.
    Text,
}

impl SearchMode {
    fn placeholder(self) -> &'static str {
        match self {
            SearchMode::Names => "Search files",
            SearchMode::Text => "Search text in files",
        }
    }
}

type PathHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type RetryHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct FileTree {
    id: ElementId,
    entries: Vec<FileEntry>,
    expanded: Vec<SharedString>,
    selected: Option<SharedString>,
    show_ignored: bool,
    search_mode: SearchMode,
    query: Option<SharedString>,
    toolbar: bool,
    autofocus: bool,
    loading: bool,
    error: Option<SharedString>,
    load_delay: Duration,
    width: f32,
    height: f32,
    on_open: Option<PathHandler>,
    on_select: Option<PathHandler>,
    on_toggle: Option<ToggleHandler>,
    on_retry: Option<RetryHandler>,
}

impl FileTree {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            entries: Vec::new(),
            expanded: Vec::new(),
            selected: None,
            show_ignored: false,
            search_mode: SearchMode::Names,
            query: None,
            toolbar: true,
            autofocus: false,
            loading: false,
            error: None,
            load_delay: DEFAULT_LOAD_DELAY,
            width: DEFAULT_WIDTH,
            height: 560.0,
            on_open: None,
            on_select: None,
            on_toggle: None,
            on_retry: None,
        }
    }

    /// A top-level entry of the workspace.
    pub fn entry(mut self, entry: FileEntry) -> Self {
        self.entries.push(entry);
        self
    }

    pub fn entries(mut self, entries: impl IntoIterator<Item = FileEntry>) -> Self {
        self.entries.extend(entries);
        self
    }

    /// Folders open on mount, by path (`"src/routes"`).
    pub fn expanded<S: Into<SharedString>>(mut self, paths: impl IntoIterator<Item = S>) -> Self {
        self.expanded.extend(paths.into_iter().map(Into::into));
        self
    }

    /// The entry selected on mount, by path.
    pub fn selected(mut self, path: impl Into<SharedString>) -> Self {
        self.selected = Some(path.into());
        self
    }

    /// Starts with hidden and ignored entries shown (Pink's "Show all files").
    pub fn show_ignored(mut self, show: bool) -> Self {
        self.show_ignored = show;
        self
    }

    pub fn search_mode(mut self, mode: SearchMode) -> Self {
        self.search_mode = mode;
        self
    }

    /// Starts with this text in the search field.
    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Shows the search toolbar (the default) or just the tree.
    pub fn toolbar(mut self, toolbar: bool) -> Self {
        self.toolbar = toolbar;
        self
    }

    /// Focuses the tree on mount, so the selection takes the accent.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// The workspace is still listing: Pink's loader in place of the tree.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// The workspace could not be listed: the message and a Retry button.
    pub fn error(mut self, message: impl Into<SharedString>) -> Self {
        self.error = Some(message.into());
        self
    }

    /// How long lazy, failing and paged folders take to list.
    pub fn load_delay(mut self, delay: Duration) -> Self {
        self.load_delay = delay;
        self
    }

    /// The pane's width (Pink's tree: 220 to 360, 286 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The pane's height; the tree scrolls inside it.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// A file was opened: clicked, or Enter on it.
    pub fn on_open(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(f));
        self
    }

    /// The selection moved to another entry.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// A folder opened (`true`) or closed.
    pub fn on_toggle(
        mut self,
        f: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }

    /// Retry on the workspace error.
    pub fn on_retry(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(f));
        self
    }
}

// ---------------------------------------------------------------------------
// File-type artwork (Pink: file_icons.rs over Symbols' file-icons.json)
// ---------------------------------------------------------------------------

/// Exact file names, from Pink's manifest (`fileNames`), trimmed to the
/// artwork this port carries.
const FILE_NAMES: &str = "\
    .claude=claude .claude.md=claude .claudeignore=claude .clauderc=claude .direnv=gear \
    .env=gear .env.dev=gear .env.development=gear .env.local=gear .env.prod=gear \
    .env.production=gear .env.test=gear .esmrc=node .git-blame-ignore=git \
    .gitattributes=git .gitconfig=git .gitignore=git .gitignore-global=git \
    .gitignore_global=git .gitinclude=git .gitkeep=git .gitmodules=git \
    .node-version=node .nvmrc=node .pnpmfile.cjs=pnpm .pylintrc=python \
    .python-version=python claude-prompt.md=claude claude.json=claude claude.md=claude \
    claude.yaml=claude claude.yml=claude commit-msg=shell dockerfile=docker \
    dockerfile.dev=docker dockerfile.prod=docker file.config=gear git-history=git \
    license=license license.md=license license.txt=license manifest.in=python \
    package-lock.json=node package.json=node pipfile=python pnpm-lock.yaml=pnpm \
    pnpm-workspace.yaml=pnpm post-merge=shell pre-commit=shell pre-push=shell \
    prisma.yml=prisma pylintrc=python pyproject.toml=gear requirements.txt=python \
    robots.txt=text setup.cfg=python test.ts=ts-test tsconfig.app.json=tsconfig \
    tsconfig.base.json=tsconfig tsconfig.build.json=tsconfig tsconfig.json=tsconfig \
    tsconfig.node.json=tsconfig tsconfig.test.json=ts-test vite.config.cjs=vite \
    vite.config.js=vite vite.config.mjs=vite vite.config.mts=vite vite.config.ts=vite";

/// Extensions, compound ones included (Pink's `fileExtensions`).
const FILE_EXTENSIONS: &str = "\
    avif=image bash=shell bat=shell bmp=image cjs=js cmd=shell css=brackets-purple \
    csv=csv db=database dockerfile=docker dockerignore=docker env=gear env.example=gear \
    fish=shell go=go heic=image htm=code-orange html=code-orange ico=image jpeg=image \
    jpg=image js=js json=brackets-yellow jsx=react lock=lock md=markdown mjs=js \
    pgsql=database png=image prisma=prisma ps1=shell psql=database py=python rs=rust \
    sh=shell shtml=code-orange spec.ts=ts-test sql=database sqlite=database \
    sqlite3=database svg=svg test.ts=ts-test tif=image tiff=image toml=gear ts=ts \
    tsconfig.json=tsconfig tsv=csv tsx=react-ts txt=text webp=image xls=csv xlsx=csv \
    yaml=yaml yaml.dist=yaml yml=yaml yml.dist=yaml zsh=shell";

/// Folder names (Pink's `folderNames`).
const FOLDER_NAMES: &str = "\
    .claude=folder-claude .claude-code=folder-claude .git=folder-red \
    .github=folder-github .turbo=folder-red api=folder-red app=folder-app \
    apps=folder-app assets=folder-assets claude=folder-claude \
    claude-config=folder-claude claude-prompts=folder-claude \
    components=folder-green-code config=folder-config css=folder-purple-code \
    database=folder-database databases=folder-database db=folder-database \
    dist=folder-purple-outline doc=folder-documents docs=folder-documents \
    documentation=folder-documents documents=folder-documents image=folder-images \
    images=folder-images lib=folder-utils node_modules=folder-node-modules \
    out=folder-purple-outline prisma=folder-prisma public=folder-purple-outline \
    resources=folder-assets router=folder-router routers=folder-router \
    routes=folder-router scripts=folder-red-code service=folder-services \
    services=folder-services source=folder-orange-code spec=folder-red-code \
    specs=folder-red-code src=folder-orange-code static=folder-assets \
    styles=folder-purple-code target=folder-target test=folder-red-code \
    tests=folder-red-code tools=folder-utils types=folder-blue-code util=folder-utils \
    utilities=folder-utils utility=folder-utils utils=folder-utils";

/// Artwork whose palette Pink lifts for dark mode (`dark_icon_svg`); these
/// ship a `-dark` twin.
const LIFTED_FOR_DARK: &str = "\
    document ts ts-test react-ts rust gear lock pnpm tsconfig brackets-purple \
    code-orange text license vite folder folder-github folder-node-modules \
    folder-orange-code folder-router folder-services folder-utils folder-red-code \
    folder-documents folder-purple-outline folder-prisma folder-green-code \
    folder-assets folder-images folder-config folder-database folder-target \
    folder-blue-code folder-purple-code folder-red folder-app folder-claude";

fn lookup(table: &'static str, key: &str) -> Option<&'static str> {
    table.split_whitespace().find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then_some(v)
    })
}

/// The artwork for an entry (Pink: file_icons.rs `asset_path`): exact names
/// win, then the longest compound extension, then the generic document or
/// folder. A folder keeps its artwork open or closed.
fn icon_asset(name: &str, kind: EntryKind) -> &'static str {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let name = name.to_ascii_lowercase();
    match kind {
        EntryKind::Folder => lookup(FOLDER_NAMES, &name).unwrap_or("folder"),
        EntryKind::File | EntryKind::Symlink => {
            if let Some(asset) = lookup(FILE_NAMES, &name) {
                return asset;
            }
            name.match_indices('.')
                .map(|(ix, _)| &name[ix + 1..])
                .filter(|ext| !ext.is_empty())
                .find_map(|ext| lookup(FILE_EXTENSIONS, ext))
                .unwrap_or("document")
        }
    }
}

fn icon_path(name: &str, kind: EntryKind, dark: bool) -> SharedString {
    let asset = icon_asset(name, kind);
    let lifted = dark && LIFTED_FOR_DARK.split_whitespace().any(|a| a == asset);
    format!(
        "icons/file-tree/file-{asset}{}.svg",
        if lifted { "-dark" } else { "" }
    )
    .into()
}

/// Polychrome artwork goes through `img` (an `svg()` is a one-color mask).
fn file_icon(name: &str, kind: EntryKind, t: &Pink) -> impl IntoElement {
    file_row::artwork(img(icon_path(name, kind, t.dark)))
}

// ---------------------------------------------------------------------------
// Model (Pink: files/model.rs `FileTreeModel`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Node {
    name: SharedString,
    kind: EntryKind,
    /// Its own flag or an ignored ancestor's (Pink `apply_page`).
    ignored: bool,
    text: Option<SharedString>,
    /// Sorted: folders, files, links; then by name.
    children: Vec<String>,
    /// The failure its next listing reports.
    fails_with: Option<SharedString>,
    page: Option<usize>,
}

/// A folder's listing as it stands.
#[derive(Clone, Debug, PartialEq)]
enum Listing {
    Unloaded,
    /// Listing; `shown` entries are already in.
    Loading {
        shown: usize,
    },
    Loaded {
        shown: usize,
    },
    Failed(SharedString),
}

#[derive(Clone, Debug, PartialEq)]
enum RowKind {
    Entry,
    Loading,
    Empty,
    Error(SharedString),
    LoadMore,
}

/// One visible row (Pink `VisibleTreeRow`). Status rows carry their folder's
/// path.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    path: String,
    depth: usize,
    kind: RowKind,
}

fn parent_path(path: &str) -> Option<String> {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .or_else(|| (!path.is_empty()).then(String::new))
}

fn is_descendant(candidate: &str, ancestor: &str) -> bool {
    if ancestor.is_empty() {
        return !candidate.is_empty();
    }
    candidate
        .strip_prefix(ancestor)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

#[derive(Clone, Debug, Default)]
struct Model {
    nodes: HashMap<String, Node>,
    listings: HashMap<String, Listing>,
    expanded: HashSet<String>,
    selected: Option<String>,
    include_ignored: bool,
}

impl Model {
    fn new(entries: &[FileEntry]) -> Self {
        let mut model = Model::default();
        model.nodes.insert(
            String::new(),
            Node {
                name: SharedString::default(),
                kind: EntryKind::Folder,
                ignored: false,
                text: None,
                children: Vec::new(),
                fails_with: None,
                page: None,
            },
        );
        model
            .listings
            .insert(String::new(), Listing::Loaded { shown: usize::MAX });
        model.add_children("", entries, false);
        model
    }

    fn add_children(&mut self, dir: &str, entries: &[FileEntry], ignored: bool) {
        let mut children = Vec::new();
        for entry in entries {
            let path = join(dir, &entry.name);
            if self.nodes.contains_key(&path) {
                continue;
            }
            let ignored = ignored || entry.ignored;
            let (fails_with, page, listing) = match &entry.load {
                FolderLoad::Ready => (None, None, Listing::Loaded { shown: usize::MAX }),
                FolderLoad::Lazy => (None, None, Listing::Unloaded),
                FolderLoad::Error(message) => (Some(message.clone()), None, Listing::Unloaded),
                FolderLoad::Paged(page) => (None, Some(*page), Listing::Unloaded),
            };
            self.nodes.insert(
                path.clone(),
                Node {
                    name: entry.name.clone(),
                    kind: entry.kind,
                    ignored,
                    text: entry.text.clone(),
                    children: Vec::new(),
                    fails_with,
                    page,
                },
            );
            if entry.kind == EntryKind::Folder {
                self.listings.insert(path.clone(), listing);
                self.add_children(&path, &entry.children, ignored);
            }
            children.push(path);
        }
        // Pink: files/model.rs `compare_paths`.
        children.sort_by(|a, b| {
            let (a, b) = (&self.nodes[a], &self.nodes[b]);
            a.kind
                .rank()
                .cmp(&b.kind.rank())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        if let Some(node) = self.nodes.get_mut(dir) {
            node.children = children;
        }
    }

    fn node(&self, path: &str) -> Option<&Node> {
        self.nodes.get(path)
    }

    fn is_folder(&self, path: &str) -> bool {
        self.node(path).is_some_and(|n| n.kind == EntryKind::Folder)
    }

    fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    fn listing(&self, dir: &str) -> Listing {
        self.listings.get(dir).cloned().unwrap_or(Listing::Unloaded)
    }

    /// What the host would list: hidden and ignored entries only on request.
    fn listed(&self, dir: &str) -> Vec<&String> {
        self.node(dir)
            .map(|node| {
                node.children
                    .iter()
                    .filter(|path| self.include_ignored || !self.nodes[*path].ignored)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The entries in so far.
    fn shown(&self, dir: &str) -> Vec<&String> {
        let listed = self.listed(dir);
        let count = match self.listing(dir) {
            Listing::Loaded { shown } | Listing::Loading { shown } => shown,
            Listing::Unloaded | Listing::Failed(_) => 0,
        };
        listed.into_iter().take(count).collect()
    }

    /// Pink: files/model.rs `rebuild_visible_rows` / `append_directory_rows`.
    fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.append_rows("", 0, &mut rows);
        rows
    }

    fn append_rows(&self, dir: &str, depth: usize, rows: &mut Vec<Row>) {
        for child in self.shown(dir) {
            rows.push(Row {
                path: child.clone(),
                depth,
                kind: RowKind::Entry,
            });
            if self.is_folder(child) && self.is_expanded(child) {
                self.append_rows(child, depth + 1, rows);
            }
        }
        if dir.is_empty() {
            return;
        }
        let status = |kind| Row {
            path: dir.to_string(),
            depth,
            kind,
        };
        match self.listing(dir) {
            Listing::Unloaded => {}
            Listing::Loading { .. } => rows.push(status(RowKind::Loading)),
            Listing::Loaded { shown } => {
                let listed = self.listed(dir).len();
                if listed == 0 {
                    rows.push(status(RowKind::Empty));
                } else if shown < listed {
                    rows.push(status(RowKind::LoadMore));
                }
            }
            Listing::Failed(message) => rows.push(status(RowKind::Error(message))),
        }
    }

    /// Opens or closes a folder (Pink `toggle_expanded`); closing over the
    /// selection moves it to the folder. `Some(open)` when it toggled.
    fn toggle(&mut self, path: &str) -> Option<bool> {
        if !self.is_folder(path) {
            return None;
        }
        if self.expanded.remove(path) {
            if self
                .selected
                .as_deref()
                .is_some_and(|selected| is_descendant(selected, path))
            {
                self.selected = Some(path.to_string());
            }
            Some(false)
        } else {
            self.expanded.insert(path.to_string());
            Some(true)
        }
    }

    /// Whether opening this folder has to list it first (Pink: tree.rs
    /// `toggle_tree_directory`).
    fn needs_load(&self, path: &str) -> bool {
        self.is_folder(path) && matches!(self.listing(path), Listing::Unloaded | Listing::Failed(_))
    }

    fn begin_load(&mut self, dir: &str) {
        let shown = match self.listing(dir) {
            Listing::Loaded { shown } => shown,
            _ => 0,
        };
        self.listings
            .insert(dir.to_string(), Listing::Loading { shown });
    }

    /// The listing arrived: a page, everything, or the folder's failure.
    fn finish_load(&mut self, dir: &str) {
        let Listing::Loading { shown } = self.listing(dir) else {
            return;
        };
        let Some(node) = self.nodes.get_mut(dir) else {
            return;
        };
        let next = if let Some(message) = node.fails_with.take() {
            Listing::Failed(message)
        } else if let Some(page) = node.page {
            Listing::Loaded {
                shown: shown.saturating_add(page),
            }
        } else {
            Listing::Loaded { shown: usize::MAX }
        };
        self.listings.insert(dir.to_string(), next);
    }

    /// Lists every ancestor of `path` and opens them (Pink
    /// `reveal_search_result`).
    fn reveal(&mut self, path: &str) {
        let mut current = parent_path(path);
        while let Some(dir) = current {
            if dir.is_empty() {
                break;
            }
            self.listings
                .insert(dir.clone(), Listing::Loaded { shown: usize::MAX });
            if let Some(node) = self.nodes.get_mut(&dir) {
                node.fails_with = None;
            }
            self.expanded.insert(dir.clone());
            current = parent_path(&dir);
        }
        self.selected = Some(path.to_string());
    }

    /// The selection must stay on a visible row (Pink `rebuild_visible_rows`).
    fn settle_selection(&mut self, rows: &[Row]) {
        if let Some(selected) = self.selected.as_deref()
            && !rows
                .iter()
                .any(|row| row.kind == RowKind::Entry && row.path == selected)
        {
            self.selected = None;
        }
    }

    /// Pink `move_selection`: one step through the visible entries; from
    /// nothing, Down starts at the top and Up at the bottom.
    fn step(&mut self, delta: isize) {
        let rows = self.rows();
        let entries: Vec<&str> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Entry)
            .map(|row| row.path.as_str())
            .collect();
        if entries.is_empty() {
            self.selected = None;
            return;
        }
        let current = self
            .selected
            .as_deref()
            .and_then(|path| entries.iter().position(|e| *e == path));
        let next = match (current, delta < 0) {
            (Some(ix), true) => ix.saturating_sub(delta.unsigned_abs()),
            (Some(ix), false) => (ix + delta as usize).min(entries.len() - 1),
            (None, true) => entries.len() - 1,
            (None, false) => 0,
        };
        self.selected = Some(entries[next].to_string());
    }

    /// Pink `select_parent`: top-level entries stay put.
    fn select_parent(&mut self) {
        if let Some(parent) = self.selected.as_deref().and_then(parent_path)
            && !parent.is_empty()
        {
            self.selected = Some(parent);
        }
    }

    /// Pink `select_first_child`.
    fn select_first_child(&mut self) {
        if let Some(first) = self
            .selected
            .clone()
            .and_then(|dir| self.shown(&dir).first().map(|p| (*p).clone()))
        {
            self.selected = Some(first);
        }
    }
}

// ---------------------------------------------------------------------------
// Name search (Pink: files/search.rs `SearchTreeModel`)
// ---------------------------------------------------------------------------

/// A name search hit (Pink: proto `WorkspaceFileSearchMatch`).
#[derive(Clone, Debug, PartialEq)]
struct NameMatch {
    path: String,
    name: SharedString,
    kind: EntryKind,
    score: i64,
}

/// Case-insensitive name matching: a run inside the name beats one in the
/// path, which beats letters scattered in order; earlier and tighter wins.
fn match_score(query: &str, name: &str, path: &str) -> Option<i64> {
    let query = query.to_lowercase();
    let name = name.to_lowercase();
    let path = path.to_lowercase();
    if let Some(at) = name.find(&query) {
        let prefix = if at == 0 { 200 } else { 0 };
        return Some(1000 + prefix - at as i64 * 4 - (name.len() - query.len()) as i64);
    }
    if let Some(at) = path.find(&query) {
        return Some(600 - at as i64 - (path.len() - query.len()) as i64 / 4);
    }
    let gaps = |haystack: &str| -> Option<i64> {
        let mut chars = haystack.char_indices();
        let mut last: Option<usize> = None;
        let mut spread = 0i64;
        for q in query.chars() {
            let (ix, _) = chars.find(|(_, c)| *c == q)?;
            if let Some(last) = last {
                spread += (ix - last - 1) as i64;
            }
            last = Some(ix);
        }
        Some(spread)
    };
    if let Some(spread) = gaps(&name) {
        return Some(400 - spread * 8);
    }
    gaps(&path).map(|spread| 100 - spread * 2)
}

fn name_matches(model: &Model, query: &str) -> Vec<NameMatch> {
    let mut found: Vec<NameMatch> = model
        .nodes
        .iter()
        .filter(|(path, node)| !path.is_empty() && (model.include_ignored || !node.ignored))
        .filter_map(|(path, node)| {
            let score = match_score(query, &node.name, path)?;
            Some(NameMatch {
                path: path.clone(),
                name: node.name.clone(),
                kind: node.kind,
                score,
            })
        })
        .collect();
    found.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    found.truncate(SEARCH_RESULT_LIMIT);
    found
}

#[derive(Clone, Debug)]
struct SearchNode {
    path: String,
    name: SharedString,
    kind: EntryKind,
    score: Option<i64>,
    best: i64,
    children: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct SearchRow {
    path: String,
    name: SharedString,
    kind: EntryKind,
    depth: usize,
    has_children: bool,
}

/// The matches arranged under their folders, best branch first.
#[derive(Clone, Debug, Default)]
struct SearchTree {
    nodes: HashMap<String, SearchNode>,
    roots: Vec<String>,
    collapsed: HashSet<String>,
    rows: Vec<SearchRow>,
    count: usize,
}

impl SearchTree {
    /// Pink: `SearchTreeModel::rebuild`.
    fn rebuild(&mut self, results: &[NameMatch]) {
        *self = SearchTree {
            count: results.len(),
            ..SearchTree::default()
        };
        for result in results {
            let mut parent: Option<String> = None;
            let mut path = String::new();
            for component in result.path.split('/').filter(|c| !c.is_empty()) {
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(component);
                let is_match = path == result.path;
                self.nodes
                    .entry(path.clone())
                    .and_modify(|node| {
                        if is_match {
                            node.name = result.name.clone();
                            node.kind = result.kind;
                            node.score = Some(node.score.unwrap_or(i64::MIN).max(result.score));
                        }
                    })
                    .or_insert_with(|| SearchNode {
                        path: path.clone(),
                        name: if is_match {
                            result.name.clone()
                        } else {
                            component.to_string().into()
                        },
                        kind: if is_match {
                            result.kind
                        } else {
                            EntryKind::Folder
                        },
                        score: is_match.then_some(result.score),
                        best: result.score,
                        children: Vec::new(),
                    });
                match &parent {
                    Some(parent) => {
                        let children = &mut self.nodes.get_mut(parent).unwrap().children;
                        if !children.contains(&path) {
                            children.push(path.clone());
                        }
                    }
                    None => {
                        if !self.roots.contains(&path) {
                            self.roots.push(path.clone());
                        }
                    }
                }
                parent = Some(path.clone());
            }
        }
        for root in self.roots.clone() {
            self.update_best(&root);
        }
        let mut roots = std::mem::take(&mut self.roots);
        self.sort(&mut roots);
        for root in &roots {
            self.sort_branch(root);
        }
        self.roots = roots;
        self.rebuild_rows();
    }

    fn update_best(&mut self, path: &str) -> i64 {
        let Some(node) = self.nodes.get(path) else {
            return i64::MIN;
        };
        let children = node.children.clone();
        let mut best = node.score.unwrap_or(i64::MIN);
        for child in children {
            best = best.max(self.update_best(&child));
        }
        if let Some(node) = self.nodes.get_mut(path) {
            node.best = best;
        }
        best
    }

    fn sort_branch(&mut self, path: &str) {
        let mut children = self
            .nodes
            .get(path)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        for child in &children {
            self.sort_branch(child);
        }
        self.sort(&mut children);
        if let Some(node) = self.nodes.get_mut(path) {
            node.children = children;
        }
    }

    /// Pink `sort_search_paths`.
    fn sort(&self, paths: &mut [String]) {
        paths.sort_by(|a, b| {
            let (a, b) = (&self.nodes[a], &self.nodes[b]);
            b.best
                .cmp(&a.best)
                .then_with(|| (b.kind == EntryKind::Folder).cmp(&(a.kind == EntryKind::Folder)))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.path.cmp(&b.path))
        });
    }

    fn rebuild_rows(&mut self) {
        let mut rows = Vec::new();
        for root in &self.roots {
            self.append_rows(root, 0, &mut rows);
        }
        self.rows = rows;
    }

    fn append_rows(&self, path: &str, depth: usize, rows: &mut Vec<SearchRow>) {
        let Some(node) = self.nodes.get(path) else {
            return;
        };
        let has_children = !node.children.is_empty();
        rows.push(SearchRow {
            path: node.path.clone(),
            name: node.name.clone(),
            kind: node.kind,
            depth,
            has_children,
        });
        if has_children && !self.collapsed.contains(path) {
            for child in &node.children {
                self.append_rows(child, depth + 1, rows);
            }
        }
    }

    fn toggle(&mut self, path: &str) -> bool {
        if !self
            .nodes
            .get(path)
            .is_some_and(|node| !node.children.is_empty())
        {
            return false;
        }
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.to_string());
        }
        self.rebuild_rows();
        true
    }
}

// ---------------------------------------------------------------------------
// Text search (Pink: files/text_search.rs)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct TextMatch {
    line: usize,
    preview: String,
    range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
struct TextFile {
    path: String,
    matches: Vec<TextMatch>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TextRow {
    File(usize),
    Match(usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
enum TextPhase {
    #[default]
    Idle,
    Searching,
    Ready,
}

#[derive(Clone, Debug, Default)]
struct TextResults {
    phase: TextPhase,
    files: Vec<TextFile>,
    scanned: usize,
    collapsed: HashSet<String>,
    rows: Vec<TextRow>,
}

impl TextResults {
    fn match_count(&self) -> usize {
        self.files.iter().map(|f| f.matches.len()).sum()
    }

    fn rebuild_rows(&mut self) {
        let mut rows = Vec::new();
        for (fx, file) in self.files.iter().enumerate() {
            rows.push(TextRow::File(fx));
            if !self.collapsed.contains(&file.path) {
                rows.extend((0..file.matches.len()).map(|mx| TextRow::Match(fx, mx)));
            }
        }
        self.rows = rows;
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// Pink: files/text_search.rs `text_search_summary`.
fn text_summary(results: &TextResults) -> SharedString {
    match results.phase {
        TextPhase::Idle => "".into(),
        TextPhase::Searching => "Searching file contents…".into(),
        TextPhase::Ready => {
            let count = results.match_count();
            if count == 0 {
                format!("No matches in {}", plural(results.scanned, "file", "files")).into()
            } else {
                format!(
                    "{} in {}",
                    plural(count, "match", "matches"),
                    plural(results.files.len(), "file", "files")
                )
                .into()
            }
        }
    }
}

/// Literal search through every file's text, in tree order; smart case.
fn text_matches(model: &Model, query: &str) -> (Vec<TextFile>, usize) {
    let case_sensitive = query.chars().any(char::is_uppercase);
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };
    let mut files = Vec::new();
    let mut scanned = 0;
    let mut stack = vec![String::new()];
    let mut order = Vec::new();
    while let Some(dir) = stack.pop() {
        let Some(node) = model.node(&dir) else {
            continue;
        };
        for child in node.children.iter().rev() {
            let entry = &model.nodes[child];
            if !model.include_ignored && entry.ignored {
                continue;
            }
            if entry.kind == EntryKind::Folder {
                stack.push(child.clone());
            }
        }
        for child in &node.children {
            let entry = &model.nodes[child];
            if (model.include_ignored || !entry.ignored) && entry.kind != EntryKind::Folder {
                order.push(child.clone());
            }
        }
    }
    for path in order {
        let node = &model.nodes[&path];
        scanned += 1;
        let Some(text) = node.text.as_ref() else {
            continue;
        };
        let mut matches = Vec::new();
        for (ix, line) in text.lines().enumerate() {
            let haystack = if case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            let Some(at) = haystack.find(&needle) else {
                continue;
            };
            let trimmed = line.trim_start();
            let cut = line.len() - trimmed.len();
            let start = at.saturating_sub(cut);
            matches.push(TextMatch {
                line: ix + 1,
                preview: trimmed.trim_end().to_string(),
                range: start..start + needle.len(),
            });
        }
        if !matches.is_empty() {
            files.push(TextFile { path, matches });
        }
    }
    (files, scanned)
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The floating rail's hover and drag state (Pink: popover.rs
/// `MenuScrollbarState`): it paints only after scroll motion, lingers, then
/// fades.
#[derive(Default)]
struct Rail {
    last_offset: Option<f32>,
    last_motion: Option<Instant>,
    bar_hovered: bool,
    grab: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RailMetrics {
    track_height: f32,
    thumb_top: f32,
    thumb_height: f32,
    max_scroll: f32,
}

impl RailMetrics {
    /// Pink `MenuScrollbarMetrics::from_viewport`.
    fn from_viewport(viewport: f32, max_scroll: f32, current: f32) -> Option<Self> {
        let max_scroll = max_scroll.max(0.0);
        if viewport <= 0.0 || max_scroll <= 0.0 {
            return None;
        }
        let track_height = (viewport - RAIL_TRACK_INSET * 2.0).max(0.0);
        if track_height <= 0.0 {
            return None;
        }
        let thumb_height = (track_height * viewport / (viewport + max_scroll))
            .max(RAIL_MIN_THUMB)
            .min(track_height);
        let travel = (track_height - thumb_height).max(0.0);
        Some(Self {
            track_height,
            thumb_top: travel * current.clamp(0.0, max_scroll) / max_scroll,
            thumb_height,
            max_scroll,
        })
    }

    fn travel(self) -> f32 {
        (self.track_height - self.thumb_height).max(0.0)
    }
}

impl Rail {
    fn note(&mut self, offset: f32) {
        match self.last_offset {
            None => self.last_offset = Some(offset),
            Some(previous) if previous != offset => {
                self.last_offset = Some(offset);
                self.last_motion = Some(Instant::now());
            }
            Some(_) => {}
        }
    }

    fn active(&self) -> bool {
        self.bar_hovered || self.grab.is_some()
    }

    fn elapsed_ms(&self) -> Option<f32> {
        self.last_motion
            .map(|at| at.elapsed().as_secs_f32() * 1000.0)
    }

    /// 1 → 0 across the fade once motion stops; full while held.
    fn fade(&self) -> f32 {
        if self.active() {
            return 1.0;
        }
        self.elapsed_ms().map_or(0.0, |elapsed| {
            (1.0 - (elapsed - RAIL_LINGER_MS) / RAIL_FADE_MS).clamp(0.0, 1.0)
        })
    }

    fn counting_down(&self) -> bool {
        !self.active()
            && self
                .elapsed_ms()
                .is_some_and(|elapsed| elapsed < RAIL_LINGER_MS + RAIL_FADE_MS)
    }

    fn visible(&self) -> bool {
        self.active() || self.counting_down()
    }

    fn drag_target(&self, metrics: &RailMetrics, pointer_in_track: f32) -> Option<f32> {
        let grab = self.grab?;
        let top = (pointer_in_track - grab).clamp(0.0, metrics.travel());
        Some(if metrics.travel() <= 0.0 {
            0.0
        } else {
            top / metrics.travel()
        })
    }
}

/// The rail's drag payload; the thumb itself is what moves.
struct RailDrag;

struct RailGhost;

impl Render for RailGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

#[derive(Default, Clone)]
struct Handlers {
    on_open: Option<PathHandler>,
    on_select: Option<PathHandler>,
    on_toggle: Option<ToggleHandler>,
}

/// What a user action asks the host's callbacks to hear.
#[derive(Default)]
struct Effects {
    opened: Option<SharedString>,
    selected: Option<SharedString>,
    toggled: Option<(SharedString, bool)>,
}

impl Effects {
    fn run(self, handlers: &Handlers, window: &mut Window, cx: &mut App) {
        if let (Some(path), Some(f)) = (self.selected, handlers.on_select.as_ref()) {
            f(&path, window, cx);
        }
        if let (Some((path, open)), Some(f)) = (self.toggled, handlers.on_toggle.as_ref()) {
            f(&path, open, window, cx);
        }
        if let (Some(path), Some(f)) = (self.opened, handlers.on_open.as_ref()) {
            f(&path, window, cx);
        }
    }
}

struct TreeState {
    model: Model,
    focus: FocusHandle,
    scroll: ScrollHandle,
    results_scroll: ScrollHandle,
    input: Entity<TextInput>,
    mode: SearchMode,
    /// The trimmed text in the field.
    query: String,
    names: SearchTree,
    /// The name results answer an earlier query (Pink keeps them up until
    /// the new answer lands).
    names_pending: bool,
    text: TextResults,
    /// The keyboard cursor in the results.
    active: usize,
    load_delay: Duration,
    rail: Rail,
    handlers: Handlers,
    search_task: Option<Task<()>>,
    load_tasks: HashMap<String, Task<()>>,
    _subscription: Subscription,
}

impl TreeState {
    fn new(config: &FileTree, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = config.search_mode;
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder(mode.placeholder()));
        let subscription = cx.subscribe_in(&input, window, |s: &mut Self, _, event, window, cx| {
            s.on_input(event, window, cx)
        });
        let mut model = Model::new(&config.entries);
        model.include_ignored = config.show_ignored;
        let focus = cx.focus_handle().tab_stop(true);
        let mut state = Self {
            model,
            focus,
            scroll: ScrollHandle::new(),
            results_scroll: ScrollHandle::new(),
            input,
            mode,
            query: String::new(),
            names: SearchTree::default(),
            names_pending: false,
            text: TextResults::default(),
            active: 0,
            load_delay: config.load_delay,
            rail: Rail::default(),
            handlers: Handlers::default(),
            search_task: None,
            load_tasks: HashMap::new(),
            _subscription: subscription,
        };
        for path in &config.expanded {
            let path = path.to_string();
            if state.model.is_folder(&path) && !state.model.is_expanded(&path) {
                state.model.expanded.insert(path.clone());
                if state.model.needs_load(&path) {
                    state.start_load(path, cx);
                }
            }
        }
        state.model.selected = config
            .selected
            .as_ref()
            .map(|s| s.to_string())
            .filter(|path| state.model.node(path).is_some());
        if let Some(query) = config.query.clone() {
            state
                .input
                .update(cx, |input, cx| input.set_text(query.clone(), cx));
            state.on_query(query.trim().to_string(), true, cx);
        }
        if config.autofocus {
            let handle = state.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&handle, cx));
        }
        state
    }

    /// Lists a folder after the host's latency (Pink `load_directory`).
    fn start_load(&mut self, dir: String, cx: &mut Context<Self>) {
        self.model.begin_load(&dir);
        let delay = self.load_delay;
        let key = dir.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |s, cx| {
                s.model.finish_load(&dir);
                s.load_tasks.remove(&dir);
                let rows = s.model.rows();
                s.model.settle_selection(&rows);
                cx.notify();
            })
            .ok();
        });
        self.load_tasks.insert(key, task);
    }

    /// Pink: tree.rs `activate_tree_path` – folders open or close, files open.
    fn activate(&mut self, path: &str, cx: &mut Context<Self>) -> Effects {
        let mut effects = Effects::default();
        if self.model.node(path).is_none() {
            return effects;
        }
        if self.model.selected.as_deref() != Some(path) {
            effects.selected = Some(path.to_string().into());
        }
        self.model.selected = Some(path.to_string());
        if self.model.is_folder(path) {
            effects.toggled = self
                .toggle(path, cx)
                .map(|open| (path.to_string().into(), open));
        } else {
            effects.opened = Some(path.to_string().into());
        }
        self.reveal_selection();
        effects
    }

    fn toggle(&mut self, path: &str, cx: &mut Context<Self>) -> Option<bool> {
        let open = self.model.toggle(path)?;
        if open && self.model.needs_load(path) {
            self.start_load(path.to_string(), cx);
        }
        Some(open)
    }

    fn reveal_selection(&self) {
        let rows = self.model.rows();
        if let Some(ix) = self.model.selected.as_deref().and_then(|selected| {
            rows.iter()
                .position(|row| row.kind == RowKind::Entry && row.path == selected)
        }) {
            self.scroll.scroll_to_item(ix);
        }
    }

    /// Pink: tree.rs `on_tree_key_down`.
    fn on_key(&mut self, key: &str, cx: &mut Context<Self>) -> Option<Effects> {
        let before = self.model.selected.clone();
        let mut effects = Effects::default();
        match key {
            "up" => self.model.step(-1),
            "down" => self.model.step(1),
            "left" => {
                if let Some(path) = self.model.selected.clone() {
                    if self.model.is_expanded(&path) {
                        effects.toggled = self.toggle(&path, cx).map(|o| (path.into(), o));
                    } else {
                        self.model.select_parent();
                    }
                }
            }
            "right" => {
                if let Some(path) = self.model.selected.clone()
                    && self.model.is_folder(&path)
                {
                    if self.model.is_expanded(&path) {
                        self.model.select_first_child();
                    } else {
                        effects.toggled = self.toggle(&path, cx).map(|o| (path.into(), o));
                    }
                }
            }
            "enter" | "space" => {
                if let Some(path) = self.model.selected.clone() {
                    return Some(self.activate(&path, cx));
                }
            }
            _ => return None,
        }
        if self.model.selected != before {
            effects.selected = self.model.selected.clone().map(Into::into);
        }
        self.reveal_selection();
        Some(effects)
    }

    fn set_include_ignored(&mut self, include: bool) {
        self.model.include_ignored = include;
        let rows = self.model.rows();
        self.model.settle_selection(&rows);
        // Pink restarts a live search against the new listing.
        if !self.query.is_empty() {
            self.run_search_now();
        }
    }

    fn set_mode(&mut self, mode: SearchMode, cx: &mut Context<Self>) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        // Pink: results never carry across modes.
        self.input.update(cx, |input, cx| {
            input.set_placeholder(mode.placeholder(), cx);
            input.clear(cx);
        });
        self.on_query(String::new(), false, cx);
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => self.on_query(text.trim().to_string(), false, cx),
            InputEvent::MoveUp | InputEvent::MoveDown => {
                let len = match self.mode {
                    SearchMode::Names => self.names.rows.len(),
                    SearchMode::Text => self.text.rows.len(),
                };
                if self.query.is_empty() || len == 0 {
                    return;
                }
                self.active = if matches!(event, InputEvent::MoveUp) {
                    self.active.saturating_sub(1)
                } else {
                    (self.active + 1).min(len - 1)
                };
                self.results_scroll.scroll_to_item(self.active);
            }
            InputEvent::Submit(_) => {
                let effects = self.activate_result(cx);
                let handlers = self.handlers.clone();
                window.defer(cx, move |window, cx| effects.run(&handlers, window, cx));
            }
            InputEvent::Escape => {
                self.input.update(cx, |input, cx| input.clear(cx));
                self.on_query(String::new(), false, cx);
            }
            InputEvent::Tab => window.focus_next(cx),
            InputEvent::Focus | InputEvent::Blur => {}
        }
        cx.notify();
    }

    /// Pink `on_search_edited`: the answer lands after a short debounce; the
    /// last one stays up meanwhile.
    fn on_query(&mut self, query: String, immediate: bool, cx: &mut Context<Self>) {
        if self.query == query && !immediate {
            return;
        }
        self.query = query;
        self.active = 0;
        self.search_task = None;
        if self.query.is_empty() {
            self.names = SearchTree::default();
            self.names_pending = false;
            self.text = TextResults::default();
            cx.notify();
            return;
        }
        if immediate {
            self.run_search_now();
            return;
        }
        let delay = match self.mode {
            SearchMode::Names => {
                self.names_pending = true;
                NAME_SEARCH_DEBOUNCE
            }
            SearchMode::Text => {
                self.text.phase = TextPhase::Searching;
                TEXT_SEARCH_DEBOUNCE
            }
        };
        self.search_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |s, cx| {
                s.run_search_now();
                cx.notify();
            })
            .ok();
        }));
    }

    fn run_search_now(&mut self) {
        match self.mode {
            SearchMode::Names => {
                let found = name_matches(&self.model, &self.query);
                self.names.rebuild(&found);
                self.names_pending = false;
            }
            SearchMode::Text => {
                let (files, scanned) = text_matches(&self.model, &self.query);
                let collapsed = std::mem::take(&mut self.text.collapsed);
                self.text = TextResults {
                    phase: TextPhase::Ready,
                    files,
                    scanned,
                    collapsed,
                    rows: Vec::new(),
                };
                self.text.rebuild_rows();
            }
        }
        self.active = 0;
    }

    /// Pink `activate_search_result` / `activate_text_search_result`.
    fn activate_result(&mut self, cx: &mut Context<Self>) -> Effects {
        if self.query.is_empty() {
            return Effects::default();
        }
        match self.mode {
            SearchMode::Names => {
                let Some(row) = self.names.rows.get(self.active).cloned() else {
                    return Effects::default();
                };
                if row.kind == EntryKind::Folder && row.has_children && self.names.toggle(&row.path)
                {
                    self.active = self
                        .names
                        .rows
                        .iter()
                        .position(|r| r.path == row.path)
                        .unwrap_or(0);
                    self.results_scroll.scroll_to_item(self.active);
                    return Effects::default();
                }
                self.reveal(&row.path, row.kind != EntryKind::Folder, cx)
            }
            SearchMode::Text => match self.text.rows.get(self.active).copied() {
                Some(TextRow::File(fx)) => {
                    let path = self.text.files[fx].path.clone();
                    if !self.text.collapsed.remove(&path) {
                        self.text.collapsed.insert(path.clone());
                    }
                    self.text.rebuild_rows();
                    self.active = self
                        .text
                        .rows
                        .iter()
                        .position(|r| *r == TextRow::File(fx))
                        .unwrap_or(0);
                    self.results_scroll.scroll_to_item(self.active);
                    Effects::default()
                }
                Some(TextRow::Match(fx, _)) => {
                    let path = self.text.files[fx].path.clone();
                    let mut effects = Effects::default();
                    if self.model.selected.as_deref() != Some(path.as_str()) {
                        effects.selected = Some(path.clone().into());
                    }
                    self.model.reveal(&path);
                    effects.opened = Some(path.into());
                    effects
                }
                None => Effects::default(),
            },
        }
    }

    /// Pink `reveal_search_result`: the tree opens down to the entry, selects
    /// it and the field clears; a file opens.
    fn reveal(&mut self, path: &str, open: bool, cx: &mut Context<Self>) -> Effects {
        let mut effects = Effects::default();
        if self.model.selected.as_deref() != Some(path) {
            effects.selected = Some(path.to_string().into());
        }
        self.model.reveal(path);
        self.input.update(cx, |input, cx| input.clear(cx));
        self.on_query(String::new(), false, cx);
        self.reveal_selection();
        if open {
            effects.opened = Some(path.to_string().into());
        }
        effects
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// What every row of one render shares.
struct Ctx<'a> {
    key: &'a str,
    t: &'a Pink,
    state: &'a Entity<TreeState>,
}

/// Runs a state action from a listener, then the host's callbacks.
fn act(
    state: &Entity<TreeState>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut TreeState, &mut Window, &mut Context<TreeState>) -> Effects,
) {
    let (effects, handlers) = state.update(cx, |s, cx| {
        let effects = f(s, window, cx);
        cx.notify();
        (effects, s.handlers.clone())
    });
    effects.run(&handlers, window, cx);
}

/// One entry row (Pink: files/tree.rs `render_tree_row`, `Entry`), drawn by
/// the `pink-file-row` package: 27 tall, an inset pill indented 14 a level,
/// the chevron slot (folders turn one glyph), the artwork and the name.
/// Focused selection takes the accent; a blurred tree keeps a quiet wash.
fn tree_row(c: &Ctx, row: &Row, model: &Model, focused: bool) -> AnyElement {
    let t = c.t;
    let Some(node) = model.node(&row.path) else {
        return div().into_any_element();
    };
    let path = row.path.clone();
    let selected = model.selected.as_deref() == Some(path.as_str());
    let is_folder = node.kind == EntryKind::Folder;
    let expanded = is_folder && model.is_expanded(&path);
    let id = format!("{}-entry-{}", c.key, path);
    let state = c.state.clone();
    let base = div()
        .id(ElementId::Name(id.clone().into()))
        .role(Role::TreeItem)
        .aria_label(node.name.clone())
        .aria_selected(selected)
        .when(is_folder, |el| el.aria_expanded(expanded))
        .when(selected, |el| {
            el.bg(if focused {
                t.accent.opacity(0.22)
            } else {
                t.wash(0.08)
            })
        })
        .when(!selected, |el| {
            el.hover_wash(id, t.fill(FillTier::Tertiary))
        })
        .on_click(move |_, window, cx| {
            let path = path.clone();
            act(&state, window, cx, |s, window, cx| {
                window.focus(&s.focus, cx);
                s.activate(&path, cx)
            });
        });
    file_row::tree_entry(base, row.depth, node.ignored)
        .child(file_row::disclosure(is_folder.then(|| {
            file_row::tree_chevron(svg().path(ICON_CHEVRON).flex_none(), expanded, t.text_faint)
        })))
        .child(file_icon(&node.name, node.kind, t))
        .child(file_row::name(
            node.name.clone(),
            if selected { t.text } else { t.text_muted },
            pink::SANS.into(),
        ))
        .into_any_element()
}

/// Pink: files/tree.rs `status_row` and the `Error` / `LoadMore` rows, drawn
/// by `pink-file-row`: full width, one level deeper than the folder's
/// entries, 11 pt.
fn status_row(c: &Ctx, row: &Row, ix: usize) -> AnyElement {
    let t = c.t;
    let dir = row.path.clone();
    let state = c.state.clone();
    let base = |id: String| div().id(ElementId::Name(id.into()));
    match &row.kind {
        RowKind::Loading => file_row::status_row(
            base(format!("{}-status-{ix}", c.key)),
            row.depth,
            file_row::StatusKind::Note,
        )
        .child(file_row::status_label("Loading…", t.text_faint))
        .into_any_element(),
        RowKind::Empty => file_row::status_row(
            base(format!("{}-status-{ix}", c.key)),
            row.depth,
            file_row::StatusKind::Note,
        )
        .child(file_row::status_label(
            "Empty folder",
            t.readable_faint(0.7),
        ))
        .into_any_element(),
        RowKind::Error(message) => {
            let id = format!("{}-error-{ix}", c.key);
            file_row::status_row(
                base(id.clone())
                    .role(Role::Button)
                    .aria_label(format!("{message}. Retry"))
                    .hover_wash(id, t.fill(FillTier::Tertiary))
                    .on_click(move |_, _, cx| {
                        let dir = dir.clone();
                        state.update(cx, |s, cx| {
                            s.start_load(dir, cx);
                            cx.notify();
                        });
                    }),
                row.depth,
                file_row::StatusKind::Error,
            )
            .child(file_row::error_message(
                message.clone(),
                t.readable_tone(t.danger, 0.82),
            ))
            .child(file_row::retry_label("Retry", t.text))
            .into_any_element()
        }
        RowKind::LoadMore => {
            let id = format!("{}-more-{ix}", c.key);
            file_row::status_row(
                base(id.clone())
                    .role(Role::Button)
                    .hover_wash(id, t.fill(FillTier::Tertiary))
                    .on_click(move |_, _, cx| {
                        let dir = dir.clone();
                        state.update(cx, |s, cx| {
                            s.start_load(dir, cx);
                            cx.notify();
                        });
                    }),
                row.depth,
                file_row::StatusKind::More,
            )
            .child(file_row::status_label("Load more…", t.text_muted))
            .into_any_element()
        }
        RowKind::Entry => div().into_any_element(),
    }
}

/// One name-search row (Pink: files/search.rs `render_search_row`), drawn by
/// `pink-file-row`: full width, indented 14 a level, an 11 pt down/right
/// chevron while it holds matches, folders in the muted tone; the cursor
/// row takes Fill Secondary.
fn search_row(c: &Ctx, row: &SearchRow, ix: usize, active: bool, expanded: bool) -> AnyElement {
    let t = c.t;
    let is_folder = row.kind == EntryKind::Folder;
    let id = format!("{}-result-{ix}", c.key);
    let state = c.state.clone();
    let base = div()
        .id(ElementId::Name(id.clone().into()))
        .role(Role::TreeItem)
        .aria_label(row.name.clone())
        .aria_selected(active)
        .when(row.has_children, |el| el.aria_expanded(expanded))
        .when(active, |el| el.bg(t.fill(FillTier::Secondary)))
        .when(!active, |el| el.hover_wash(id, t.fill(FillTier::Tertiary)))
        .on_click(move |_, window, cx| {
            act(&state, window, cx, |s, _, cx| {
                s.active = ix;
                s.activate_result(cx)
            });
        });
    file_row::search_entry(base, row.depth)
        .child(file_row::disclosure(row.has_children.then(|| {
            file_row::search_chevron(
                svg().path(if expanded {
                    ICON_CHEVRON_DOWN
                } else {
                    ICON_CHEVRON
                }),
                t.text_faint,
            )
        })))
        .child(file_icon(&row.name, row.kind, t))
        .child(file_row::name(
            row.name.clone(),
            if is_folder { t.text_muted } else { t.text },
            pink::SANS.into(),
        ))
        .into_any_element()
}

/// One text-search row (Pink: files/text_search.rs `render_text_search_row`),
/// drawn by `pink-file-row`: a file row (chevron 11, artwork, name ≤ 180,
/// its folder faint, the match count 10.5) or a match row (line number 10.5
/// in a 28 pt column, the line in mono with the hit on the inline-code
/// wash).
fn text_row(c: &Ctx, results: &TextResults, row: TextRow, ix: usize, active: bool) -> AnyElement {
    let t = c.t;
    let id = format!("{}-text-{ix}", c.key);
    let state = c.state.clone();
    let base = div()
        .id(ElementId::Name(id.clone().into()))
        .role(Role::TreeItem)
        .aria_selected(active)
        .when(active, |el| el.bg(t.fill(FillTier::Secondary)))
        .when(!active, |el| el.hover_wash(id, t.fill(FillTier::Tertiary)))
        .on_click(move |_, window, cx| {
            act(&state, window, cx, |s, _, cx| {
                s.active = ix;
                s.activate_result(cx)
            });
        });
    match row {
        TextRow::File(fx) => {
            let file = &results.files[fx];
            let expanded = !results.collapsed.contains(&file.path);
            let (dir, name) = file
                .path
                .rsplit_once('/')
                .unwrap_or(("", file.path.as_str()));
            file_row::text_row(
                base.aria_label(format!(
                    "{}, {}",
                    file.path,
                    plural(file.matches.len(), "match", "matches")
                ))
                .aria_expanded(expanded),
                file_row::TextRowKind::File,
                pink::SANS.into(),
            )
            .child(file_row::search_chevron(
                svg().path(if expanded {
                    ICON_CHEVRON_DOWN
                } else {
                    ICON_CHEVRON
                }),
                t.text_faint,
            ))
            .child(file_icon(name, EntryKind::File, t))
            .child(file_row::text_file_name(name.to_string(), t.text))
            .child(file_row::text_file_folder(dir.to_string(), t.text_faint))
            .child(file_row::match_count(
                file.matches.len().to_string(),
                t.text_faint,
            ))
            .into_any_element()
        }
        TextRow::Match(fx, mx) => {
            let file = &results.files[fx];
            let found = &file.matches[mx];
            let preview = found.preview.replace('\t', " ");
            let range = found.range.start.min(preview.len())..found.range.end.min(preview.len());
            let mut styled = StyledText::new(preview.clone());
            if preview.is_char_boundary(range.start) && preview.is_char_boundary(range.end) {
                styled = styled.with_highlights([(
                    range,
                    HighlightStyle {
                        color: Some(t.code_text),
                        background_color: Some(t.code_wash),
                        ..Default::default()
                    },
                )]);
            }
            file_row::text_row(
                base.aria_label(format!(
                    "{} line {}: {}",
                    file.path,
                    found.line,
                    preview.trim()
                )),
                file_row::TextRowKind::Match,
                pink::SANS.into(),
            )
            .child(file_row::line_number(found.line.to_string(), t.text_faint))
            .child(file_row::match_line(t.text_muted, pink::MONO.into()).child(styled))
            .into_any_element()
        }
    }
}

/// A centered line in place of results (Pink `centered_search_message`).
fn centered_message(message: impl Into<SharedString>, color: Hsla) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .px(px(24.0))
        .text_center()
        .text_size(px(11.5))
        .text_color(color)
        .child(message.into())
        .into_any_element()
}

/// A toolbar control (Pink: files/mod.rs `toolbar_button`): 28 square,
/// radius 6, a quiet wash under the pointer, the inset focus ring, a
/// tooltip; pressing it keeps focus where it was. On: `wash(0.1)`.
fn toolbar_button(
    id: String,
    label: &'static str,
    glyph: &'static str,
    on: bool,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    div()
        .id(ElementId::Name(id.clone().into()))
        .size(px(chrome::CONTROL_M))
        .flex_none()
        .rounded(px(chrome::RADIUS_M))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(label)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .hover_wash(id, t.fill(FillTier::Tertiary))
        .when(on, |el| el.bg(t.wash(0.1)))
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_key_down(pink::swallow_activation_key)
        .child(
            svg()
                .path(glyph)
                .size(px(chrome::ICON_M))
                .flex_none()
                .text_color(if on { t.text } else { t.text_muted }),
        )
}

/// The floating rail over the tree (Pink: popover.rs `rail` /
/// `MenuScrollbarState::render_rail`): a 10 px hit strip at the right edge
/// with a 3 px thumb (5 under the pointer or in a drag), painted only after
/// scroll motion, lingering 1.4 s and fading over 260 ms.
fn scroll_rail(c: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let t = c.t;
    let (metrics, visible, active, fade) = c.state.update(cx, |s, _| {
        let bounds = s.scroll.bounds();
        let max = f32::from(s.scroll.max_offset().y).max(0.0);
        let offset = (-f32::from(s.scroll.offset().y)).clamp(0.0, max);
        s.rail.note(offset);
        let metrics = RailMetrics::from_viewport(f32::from(bounds.size.height), max, offset);
        (metrics, s.rail.visible(), s.rail.active(), s.rail.fade())
    });
    if c.state.read(cx).rail.counting_down() {
        window.request_animation_frame();
    }
    let metrics = metrics?;
    if !visible {
        return None;
    }
    let faint = t.text_faint;
    let state = c.state.clone();
    let press_state = c.state.clone();
    let drag_state = c.state.clone();
    let up_state = c.state.clone();
    let out_state = c.state.clone();
    Some(
        scroll_rail::rail(faint, fade, active, metrics.thumb_top, metrics.thumb_height)
            .id(ElementId::Name(format!("{}-rail", c.key).into()))
            .on_hover(move |hovered: &bool, _, cx| {
                let hovered = *hovered;
                state.update(cx, |s, cx| {
                    let active = hovered || s.rail.grab.is_some();
                    if s.rail.bar_hovered != active {
                        s.rail.bar_hovered = active;
                        if !active {
                            s.rail.last_motion = Some(Instant::now());
                        }
                        cx.notify();
                    }
                });
            })
            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                press_state.update(cx, |s, cx| {
                    let top = s.scroll.bounds().top();
                    let pointer = f32::from(event.position.y - top) - RAIL_TRACK_INSET;
                    let grab = if (metrics.thumb_top..=metrics.thumb_top + metrics.thumb_height)
                        .contains(&pointer)
                    {
                        pointer - metrics.thumb_top
                    } else {
                        metrics.thumb_height / 2.0
                    };
                    s.rail.grab = Some(grab);
                    if let Some(fraction) = s.rail.drag_target(&metrics, pointer) {
                        s.scroll
                            .set_offset(point(px(0.0), px(-fraction * metrics.max_scroll)));
                    }
                    cx.notify();
                });
            })
            .on_drag(RailDrag, |_, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| RailGhost)
            })
            .on_drag_move(move |event: &DragMoveEvent<RailDrag>, _, cx| {
                let y = event.event.position.y;
                drag_state.update(cx, |s, cx| {
                    let bounds = s.scroll.bounds();
                    let max = f32::from(s.scroll.max_offset().y).max(0.0);
                    let offset = -f32::from(s.scroll.offset().y);
                    let Some(m) =
                        RailMetrics::from_viewport(f32::from(bounds.size.height), max, offset)
                    else {
                        return;
                    };
                    let pointer = f32::from(y - bounds.top()) - RAIL_TRACK_INSET;
                    if let Some(fraction) = s.rail.drag_target(&m, pointer) {
                        s.scroll
                            .set_offset(point(px(0.0), px(-fraction * m.max_scroll)));
                        cx.notify();
                    }
                });
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                up_state.update(cx, |s, cx| {
                    s.rail.grab = None;
                    s.rail.last_motion = Some(Instant::now());
                    cx.notify();
                });
            })
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                out_state.update(cx, |s, cx| {
                    if s.rail.grab.take().is_some() {
                        s.rail.bar_hovered = false;
                        s.rail.last_motion = Some(Instant::now());
                        cx.notify();
                    }
                });
            })
            .into_any_element(),
    )
}

/// Pink: surface_chrome.rs `surface_state`, filling the pane.
fn pane_state(glyph: AnyElement, title: &str, body: Option<SharedString>, t: &Pink) -> Div {
    pink::surface_state(glyph, title.to_string(), body, t)
        .flex_1()
        .min_h_0()
}

impl RenderOnce for FileTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "file-tree".into()),
            cx,
            |window, cx| TreeState::new(&self, window, cx),
        );
        let handlers = Handlers {
            on_open: self.on_open.clone(),
            on_select: self.on_select.clone(),
            on_toggle: self.on_toggle.clone(),
        };
        let input = state.update(cx, |s, _| {
            s.handlers = handlers;
            s.input.clone()
        });
        input.update(cx, |input, _| {
            input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
        });

        let c = Ctx {
            key: &key,
            t: &t,
            state: &state,
        };
        let s = state.read(cx);
        let focused = s.focus.is_focused(window);
        let mode = s.mode;
        let include_ignored = s.model.include_ignored;
        let query_live = !s.query.is_empty();
        let focus = s.focus.clone();
        let scroll = s.scroll.clone();
        let results_scroll = s.results_scroll.clone();
        let active = s.active;

        // -- Body ---------------------------------------------------------------
        let mut show_rail = false;
        let body: AnyElement = if query_live && mode == SearchMode::Text {
            // Pink: `render_text_search_results` – the summary over the matches.
            let summary = text_summary(&s.text);
            let rows: Vec<AnyElement> = s
                .text
                .rows
                .iter()
                .enumerate()
                .map(|(ix, row)| text_row(&c, &s.text, *row, ix, ix == active))
                .collect();
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(ElementId::Name(format!("{key}-text-summary").into()))
                        .role(Role::Status)
                        .aria_label(summary.clone())
                        .min_h(px(24.0))
                        .flex_none()
                        .px(px(10.0))
                        .py(px(3.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .text_size(px(11.0))
                        .text_color(t.text_faint)
                        .child(div().min_w_0().flex_1().child(summary)),
                )
                .child(
                    div()
                        .id(ElementId::Name(format!("{key}-text-results").into()))
                        .role(Role::Tree)
                        .aria_label("Workspace text matches")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&results_scroll)
                        .flex()
                        .flex_col()
                        .children(rows),
                )
                .into_any_element()
        } else if query_live {
            // Pink: `render_search_results`.
            if s.names.rows.is_empty() {
                centered_message(
                    if s.names_pending {
                        "Searching…"
                    } else {
                        "No files found."
                    },
                    t.text_faint,
                )
            } else {
                let rows: Vec<AnyElement> = s
                    .names
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(ix, row)| {
                        // Branches start open; `collapsed` holds the closed ones.
                        let expanded = row.has_children && !s.names.collapsed.contains(&row.path);
                        search_row(&c, row, ix, ix == active, expanded)
                    })
                    .collect();
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .when(s.names.count >= SEARCH_RESULT_LIMIT, |el| {
                        el.child(
                            div()
                                .h(px(24.0))
                                .flex_none()
                                .px(px(10.0))
                                .flex()
                                .items_center()
                                .text_size(px(11.0))
                                .text_color(t.text_faint)
                                .child("Showing the first 200 matches"),
                        )
                    })
                    .child(
                        div()
                            .id(ElementId::Name(format!("{key}-results").into()))
                            .role(Role::Tree)
                            .aria_label("Fuzzy workspace file results")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&results_scroll)
                            .flex()
                            .flex_col()
                            .children(rows),
                    )
                    .into_any_element()
            }
        } else if let Some(error) = self.error.clone() {
            // Pink: files/mod.rs – "Files couldn't load" with a Retry.
            let retry_id = format!("{key}-retry");
            let on_retry = self.on_retry.clone();
            let retry = div()
                .id(ElementId::Name(retry_id.clone().into()))
                .role(Role::Button)
                .aria_label("Retry loading workspace files")
                .h(px(28.0))
                .px(px(12.0))
                .rounded(px(7.0))
                .border_1()
                .border_color(t.border)
                .hover_fill(
                    retry_id,
                    t.fill(FillTier::Quaternary),
                    t.fill(FillTier::Tertiary),
                )
                .cursor_pointer()
                .flex()
                .items_center()
                .text_size(px(12.0))
                .text_color(t.text)
                .child("Retry")
                .on_click(move |_, window, cx| {
                    if let Some(f) = on_retry.as_ref() {
                        f(window, cx);
                    }
                });
            pane_state(
                svg()
                    .path(ICON_FOLDER)
                    .size(px(28.0))
                    .text_color(t.text_faint)
                    .into_any_element(),
                "Files couldn't load",
                Some(error),
                &t,
            )
            .child(div().mt(px(6.0)).child(retry))
            .into_any_element()
        } else if self.loading {
            // Pink: "A loader instead of a blank pane (audit SS-12)".
            pane_state(
                loaders::gradient_spinner(2.5, t.glyph).into_any_element(),
                "Loading files…",
                None,
                &t,
            )
            .into_any_element()
        } else {
            show_rail = true;
            let rows = s.model.rows();
            let items: Vec<AnyElement> = rows
                .iter()
                .enumerate()
                .map(|(ix, row)| match row.kind {
                    RowKind::Entry => tree_row(&c, row, &s.model, focused),
                    _ => status_row(&c, row, ix),
                })
                .collect();
            // Pink: files/tree.rs `render_tree` – the tree takes focus on a
            // press and walks on the arrow keys.
            let key_state = state.clone();
            let press_state = state.clone();
            div()
                .id(ElementId::Name(format!("{key}-tree").into()))
                .role(Role::Tree)
                .aria_label("Workspace file tree")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .track_focus(&focus)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    let handle = press_state.read(cx).focus.clone();
                    window.focus(&handle, cx);
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.clone();
                    let handled = key_state.update(cx, |s, cx| {
                        let effects = s.on_key(&key, cx)?;
                        cx.notify();
                        Some((effects, s.handlers.clone()))
                    });
                    if let Some((effects, handlers)) = handled {
                        window.prevent_default();
                        cx.stop_propagation();
                        effects.run(&handlers, window, cx);
                    }
                })
                .flex()
                .flex_col()
                .children(items)
                .into_any_element()
        };

        // -- Toolbar (Pink: files/mod.rs `render_header`) -------------------
        let toolbar = self.toolbar.then(|| {
            let mode_state = state.clone();
            let eye_state = state.clone();
            div()
                .h(px(layout::TITLEBAR_HEIGHT))
                .w_full()
                .flex_none()
                .px(px(chrome::EDGE_INSET))
                .flex()
                .items_center()
                .gap(px(chrome::CONTROL_GAP))
                .child(
                    pink::toolbar_input(&t)
                        .child(
                            svg()
                                .path(ICON_SEARCH)
                                .size(px(12.0))
                                .flex_none()
                                .text_color(t.text_faint),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_size(px(11.0))
                                .line_height(px(16.0))
                                .text_color(t.text)
                                .child(input.clone()),
                        ),
                )
                .child(
                    toolbar_button(
                        format!("{key}-text-mode"),
                        "Search text in files",
                        ICON_TEXT_SEARCH,
                        mode == SearchMode::Text,
                        &t,
                    )
                    .on_click(move |_, _, cx| {
                        mode_state.update(cx, |s, cx| {
                            let next = if s.mode == SearchMode::Text {
                                SearchMode::Names
                            } else {
                                SearchMode::Text
                            };
                            s.set_mode(next, cx);
                            cx.notify();
                        });
                    }),
                )
                .child(
                    toolbar_button(
                        format!("{key}-ignored"),
                        if include_ignored {
                            "Hide hidden and ignored files"
                        } else {
                            "Show all files (even hidden)"
                        },
                        if include_ignored {
                            ICON_EYE
                        } else {
                            ICON_EYE_CLOSED
                        },
                        include_ignored,
                        &t,
                    )
                    .on_click(move |_, _, cx| {
                        eye_state.update(cx, |s, cx| {
                            let include = !s.model.include_ignored;
                            s.set_include_ignored(include);
                            cx.notify();
                        });
                    }),
                )
        });

        let rail = if show_rail {
            scroll_rail(&c, window, cx)
        } else {
            None
        };

        motion::keep_alive(window);
        // The Files pane on the sidebar's chrome surface.
        div()
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .overflow_hidden()
            .bg(t.chrome_surface())
            .font_family(pink::SANS)
            .text_color(t.text)
            .children(toolbar)
            .child(
                div()
                    .id(ElementId::Name(format!("{key}-pane").into()))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(body)
                    .children(rail),
            )
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

const WEBHOOKS_TS: &str = r#"import { Router } from "express";
import { stripe } from "../services/stripe";
import { markInvoicePaid, voidInvoice } from "./invoices";

export const webhooks = Router();

webhooks.post("/stripe", async (req, res) => {
  const event = stripe.webhooks.constructEvent(
    req.body,
    req.headers["stripe-signature"],
    process.env.STRIPE_WEBHOOK_SECRET,
  );
  if (event.type === "invoice.paid") {
    await markInvoicePaid(event.data.object.id);
  }
  if (event.type === "invoice.voided") {
    await voidInvoice(event.data.object.id);
  }
  res.sendStatus(200);
});
"#;

const INVOICES_TS: &str = r#"import { Router } from "express";
import { db } from "../lib/db";
import { formatMoney } from "../lib/money";

export const invoices = Router();

invoices.get("/:id", async (req, res) => {
  const invoice = await db.invoice.findUnique({ where: { id: req.params.id } });
  res.json({ ...invoice, total: formatMoney(invoice.total) });
});

export async function markInvoicePaid(id: string) {
  await db.invoice.update({ where: { id }, data: { status: "paid" } });
}

export async function voidInvoice(id: string) {
  await db.invoice.update({ where: { id }, data: { status: "void" } });
}
"#;

const STRIPE_TS: &str = r#"import Stripe from "stripe";

export const stripe = new Stripe(process.env.STRIPE_SECRET_KEY, {
  apiVersion: "2024-06-20",
});

export async function createInvoice(customer: string, items: LineItem[]) {
  const invoice = await stripe.invoices.create({ customer, auto_advance: true });
  return invoice;
}
"#;

const TAX_TS: &str = r#"export function taxFor(country: string, amount: number) {
  const rate = RATES[country] ?? 0;
  return Math.round(amount * rate);
}
"#;

// joiners: allow sample source code, not a label
const MONEY_TS: &str = r#"export function formatMoney(cents: number, currency = "EUR") {
  return new Intl.NumberFormat("en", { style: "currency", currency }).format(cents / 100);
}
"#;

fn demo_entries() -> Vec<FileEntry> {
    vec![
        FileEntry::folder(".github").child(
            FileEntry::folder("workflows")
                .children([FileEntry::file("ci.yml"), FileEntry::file("deploy.yml")]),
        ),
        FileEntry::folder("dist").ignored(true).children([
            FileEntry::file("server.js"),
            FileEntry::file("server.js.map"),
        ]),
        FileEntry::folder("docs").children([
            FileEntry::file("billing-flow.md"),
            FileEntry::file("webhooks.md")
                .text("# Webhooks\n\nStripe calls /webhooks/stripe for every invoice event.\n"),
        ]),
        FileEntry::folder("node_modules")
            .ignored(true)
            .children([FileEntry::folder("express"), FileEntry::folder("stripe")]),
        FileEntry::folder("prisma").children([
            FileEntry::folder("migrations").paged(2).children([
                FileEntry::folder("20240611_init"),
                FileEntry::folder("20240702_invoice_status"),
                FileEntry::folder("20240819_tax_rates"),
                FileEntry::folder("20240903_refunds"),
            ]),
            FileEntry::file("schema.prisma")
                .text("model Invoice {\n  id     String @id\n  status String\n  total  Int\n}\n"),
        ]),
        FileEntry::folder("src").children([
            FileEntry::folder("lib").children([
                FileEntry::file("db.ts"),
                FileEntry::file("logger.ts"),
                FileEntry::file("money.ts").text(MONEY_TS),
            ]),
            FileEntry::folder("routes").children([
                FileEntry::file("invoices.ts").text(INVOICES_TS),
                FileEntry::file("subscriptions.ts"),
                FileEntry::file("webhooks.ts").text(WEBHOOKS_TS),
            ]),
            FileEntry::folder("services").children([
                FileEntry::file("stripe.ts").text(STRIPE_TS),
                FileEntry::file("tax.ts").text(TAX_TS),
            ]),
            FileEntry::file("index.ts"),
            FileEntry::file("server.ts"),
        ]),
        FileEntry::folder("tests").lazy().children([
            FileEntry::file("invoices.test.ts"),
            FileEntry::file("webhooks.test.ts"),
        ]),
        FileEntry::file(".env.example"),
        FileEntry::file(".env.local").ignored(true),
        FileEntry::file(".gitignore"),
        FileEntry::file("CLAUDE.md"),
        FileEntry::file("Dockerfile"),
        FileEntry::file("package.json"),
        FileEntry::file("pnpm-lock.yaml"),
        FileEntry::file("README.md"),
        FileEntry::file("tsconfig.json"),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(24.0))
            .child(
                FileTree::new("file-tree")
                    .height(600.0)
                    .entries(demo_entries())
                    .show_ignored(true)
                    .expanded(["src", "src/routes", "prisma", "prisma/migrations"])
                    .selected("src/routes/webhooks.ts")
                    .autofocus(true),
            )
            .child(
                FileTree::new("file-tree-search")
                    .height(600.0)
                    .entries(demo_entries())
                    .search_mode(SearchMode::Text)
                    .query("invoice"),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use EntryKind::{File, Folder, Symlink};

    fn paths(rows: &[Row]) -> Vec<(&str, usize)> {
        rows.iter()
            .filter(|row| row.kind == RowKind::Entry)
            .map(|row| (row.path.as_str(), row.depth))
            .collect()
    }

    #[test]
    fn artwork_follows_pinks_manifest() {
        // Pink: file_icons.rs tests, for the artwork this port carries.
        assert_eq!(icon_asset("C:\\project\\PACKAGE.JSON", File), "node");
        assert_eq!(icon_asset("config/.env.local", File), "gear");
        assert_eq!(icon_asset("docs/claude.md", File), "claude");
        assert_eq!(icon_asset("folder.css/unknown", File), "document");
        assert_eq!(icon_asset("invoices.test.ts", File), "ts-test");
        assert_eq!(icon_asset("model.schema.json", File), "brackets-yellow");
        assert_eq!(icon_asset("src", Folder), "folder-orange-code");
        assert_eq!(icon_asset("unknown", Folder), "folder");
        assert_eq!(icon_asset("README.md", Symlink), "markdown");
        assert_eq!(icon_asset("unknown.unrecognized", File), "document");
        assert_eq!(icon_asset("", File), "document");
        assert_eq!(
            icon_path("main.rs", File, true),
            "icons/file-tree/file-rust-dark.svg"
        );
        assert_eq!(
            icon_path("main.rs", File, false),
            "icons/file-tree/file-rust.svg"
        );
        // Artwork Pink leaves alone in dark mode has no twin.
        assert_eq!(
            icon_path("ci.yml", File, true),
            "icons/file-tree/file-yaml.svg"
        );
    }

    #[test]
    fn every_artwork_ships_with_its_dark_twin() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons/file-tree");
        let assets: HashSet<&str> = [FILE_NAMES, FILE_EXTENSIONS, FOLDER_NAMES]
            .iter()
            .flat_map(|table| table.split_whitespace())
            .filter_map(|pair| pair.split_once('=').map(|(_, v)| v))
            .chain(["document", "folder"])
            .collect();
        for asset in assets {
            assert!(dir.join(format!("file-{asset}.svg")).exists(), "{asset}");
            let lifted = LIFTED_FOR_DARK.split_whitespace().any(|a| a == asset);
            assert_eq!(
                dir.join(format!("file-{asset}-dark.svg")).exists(),
                lifted,
                "{asset} dark twin"
            );
        }
    }

    #[test]
    fn rows_list_folders_first_and_nest() {
        let mut model = Model::new(&[
            FileEntry::file("b.ts"),
            FileEntry::folder("src").child(FileEntry::file("a.ts")),
            FileEntry::file("A.md"),
        ]);
        assert_eq!(paths(&model.rows()), [("src", 0), ("A.md", 0), ("b.ts", 0)]);
        assert_eq!(model.toggle("src"), Some(true));
        assert_eq!(
            paths(&model.rows()),
            [("src", 0), ("src/a.ts", 1), ("A.md", 0), ("b.ts", 0)]
        );
        assert_eq!(model.toggle("b.ts"), None);
    }

    #[test]
    fn ignored_entries_wait_for_the_eye_and_dim_their_children() {
        let mut model = Model::new(&demo_entries());
        let rows = model.rows();
        let top: Vec<_> = paths(&rows).into_iter().map(|(p, _)| p).collect();
        assert!(!top.contains(&"dist") && !top.contains(&".env.local"));
        model.include_ignored = true;
        model.toggle("dist");
        let rows = model.rows();
        assert!(paths(&rows).contains(&("dist/server.js", 1)));
        assert!(model.node("dist/server.js").unwrap().ignored);
        assert!(!model.node("src").unwrap().ignored);
    }

    #[test]
    fn closing_a_folder_over_the_selection_selects_it() {
        let mut model = Model::new(&demo_entries());
        model.toggle("src");
        model.toggle("src/routes");
        model.selected = Some("src/routes/webhooks.ts".into());
        model.toggle("src");
        assert_eq!(model.selected.as_deref(), Some("src"));
    }

    #[test]
    fn keyboard_steps_follow_the_visible_rows() {
        let mut model = Model::new(&[
            FileEntry::folder("src").children([
                FileEntry::file("main.ts"),
                FileEntry::folder("lib").child(FileEntry::file("db.ts")),
            ]),
            FileEntry::file("README.md"),
        ]);
        // From nothing, Down starts at the top and Up at the bottom.
        model.step(1);
        assert_eq!(model.selected.as_deref(), Some("src"));
        model.selected = None;
        model.step(-1);
        assert_eq!(model.selected.as_deref(), Some("README.md"));
        model.step(1);
        assert_eq!(model.selected.as_deref(), Some("README.md"));
        // Into an open folder; its first entry is the subfolder.
        model.toggle("src");
        model.selected = Some("src".into());
        model.select_first_child();
        assert_eq!(model.selected.as_deref(), Some("src/lib"));
        model.step(1);
        assert_eq!(model.selected.as_deref(), Some("src/main.ts"));
        model.select_parent();
        assert_eq!(model.selected.as_deref(), Some("src"));
        // A top-level entry has no parent to go to.
        model.select_parent();
        assert_eq!(model.selected.as_deref(), Some("src"));
    }

    #[test]
    fn listings_load_fail_retry_and_page() {
        let mut model = Model::new(&[
            FileEntry::folder("lazy")
                .lazy()
                .child(FileEntry::file("a.ts")),
            FileEntry::folder("locked")
                .error("Permission denied")
                .child(FileEntry::file("key.pem")),
            FileEntry::folder("paged")
                .paged(2)
                .children(["1.sql", "2.sql", "3.sql"].map(FileEntry::file)),
            FileEntry::folder("void"),
        ]);
        let kinds = |model: &Model, dir: &str| -> Vec<RowKind> {
            model
                .rows()
                .into_iter()
                .filter(|row| {
                    row.path.starts_with(dir) && row.path != dir || {
                        row.path == dir && row.kind != RowKind::Entry
                    }
                })
                .map(|row| row.kind)
                .collect()
        };
        for dir in ["lazy", "locked", "paged", "void"] {
            model.toggle(dir);
        }
        assert!(model.needs_load("lazy") && !model.needs_load("void"));
        assert_eq!(kinds(&model, "void"), [RowKind::Empty]);

        model.begin_load("lazy");
        assert_eq!(kinds(&model, "lazy"), [RowKind::Loading]);
        model.finish_load("lazy");
        assert_eq!(kinds(&model, "lazy"), [RowKind::Entry]);

        model.begin_load("locked");
        model.finish_load("locked");
        assert_eq!(
            kinds(&model, "locked"),
            [RowKind::Error("Permission denied".into())]
        );
        assert!(model.needs_load("locked"));
        model.begin_load("locked");
        model.finish_load("locked");
        assert_eq!(kinds(&model, "locked"), [RowKind::Entry]);

        model.begin_load("paged");
        model.finish_load("paged");
        assert_eq!(
            kinds(&model, "paged"),
            [RowKind::Entry, RowKind::Entry, RowKind::LoadMore]
        );
        model.begin_load("paged");
        assert_eq!(
            kinds(&model, "paged"),
            [RowKind::Entry, RowKind::Entry, RowKind::Loading]
        );
        model.finish_load("paged");
        assert_eq!(
            kinds(&model, "paged"),
            [RowKind::Entry, RowKind::Entry, RowKind::Entry]
        );
    }

    #[test]
    fn name_search_shows_matches_under_their_folders() {
        let model = Model::new(&demo_entries());
        let mut tree = SearchTree::default();
        tree.rebuild(&name_matches(&model, "webhooks"));
        let rows: Vec<_> = tree
            .rows
            .iter()
            .map(|row| (row.path.as_str(), row.depth))
            .collect();
        assert_eq!(
            rows,
            [
                ("docs", 0),
                ("docs/webhooks.md", 1),
                ("src", 0),
                ("src/routes", 1),
                ("src/routes/webhooks.ts", 2),
                ("tests", 0),
                ("tests/webhooks.test.ts", 1),
            ]
        );
        assert!(tree.toggle("src"));
        assert!(!tree.rows.iter().any(|row| row.path == "src/routes"));
        assert!(!tree.toggle("docs/webhooks.md"));
        // Ignored entries are found only when shown.
        assert!(name_matches(&model, "server.js").is_empty());
    }

    #[test]
    fn text_search_reads_like_pink() {
        let model = Model::new(&demo_entries());
        let (files, scanned) = text_matches(&model, "invoice");
        let mut results = TextResults {
            phase: TextPhase::Ready,
            files,
            scanned,
            ..TextResults::default()
        };
        results.rebuild_rows();
        assert_eq!(text_summary(&results), "18 matches in 5 files");
        assert_eq!(results.files[0].path, "docs/webhooks.md");
        let first = &results.files[2].matches[0];
        assert_eq!(first.line, 5);
        assert_eq!(&first.preview[first.range.clone()], "invoice");
        // Smart case: an uppercase letter makes it exact.
        assert_eq!(text_matches(&model, "Invoice").0.len(), 4);
        results.files.clear();
        assert_eq!(
            text_summary(&results),
            format!("No matches in {scanned} files")
        );
        results.phase = TextPhase::Searching;
        assert_eq!(text_summary(&results), "Searching file contents…");
    }

    #[test]
    fn rail_metrics_match_pink() {
        assert_eq!(RailMetrics::from_viewport(100.0, 0.0, 0.0), None);
        let m = RailMetrics::from_viewport(100.0, 100.0, 50.0).unwrap();
        assert_eq!(
            (m.track_height, m.thumb_height, m.thumb_top),
            (92.0, 46.0, 23.0)
        );
        let long = RailMetrics::from_viewport(100.0, 10_000.0, 10_000.0).unwrap();
        assert_eq!(long.thumb_height, RAIL_MIN_THUMB);
        assert_eq!(long.thumb_top, long.travel());
    }

    /// Hosts a tree and records what it reports.
    #[derive(Default)]
    struct Host {
        opened: Vec<SharedString>,
        toggled: Vec<(SharedString, bool)>,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let opened = cx.entity().downgrade();
            let toggled = opened.clone();
            div().size_full().child(
                FileTree::new("ft")
                    .toolbar(false)
                    .entry(FileEntry::folder("src").children([
                        FileEntry::folder("lib").child(FileEntry::file("db.ts")),
                        FileEntry::file("main.ts"),
                        FileEntry::file("app.ts"),
                    ]))
                    .entry(FileEntry::file("README.md"))
                    .selected("src")
                    .autofocus(true)
                    .on_open(move |path, _, cx| {
                        opened
                            .update(cx, |h, cx| {
                                h.opened.push(path.clone());
                                cx.notify();
                            })
                            .ok();
                    })
                    .on_toggle(move |path, open, _, cx| {
                        toggled
                            .update(cx, |h, cx| {
                                h.toggled.push((path.clone(), open));
                                cx.notify();
                            })
                            .ok();
                    }),
            )
        }
    }

    #[gpui::test]
    fn arrow_keys_walk_and_enter_opens(cx: &mut gpui::TestAppContext) {
        cx.update(crate::init);
        let (host, cx) = cx.add_window_view(|_, _| Host::default());
        let settle = |cx: &mut gpui::VisualTestContext| {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        };
        settle(cx);
        // Right opens the folder, Right again steps in, Down moves on.
        cx.simulate_keystrokes("right right down enter");
        settle(cx);
        host.read_with(cx, |h, _| {
            assert_eq!(h.toggled, [("src".into(), true)]);
            assert_eq!(h.opened, ["src/app.ts"]);
        });
        // Left goes up to the folder, then closes it.
        cx.simulate_keystrokes("left left");
        settle(cx);
        host.read_with(cx, |h, _| {
            assert_eq!(h.toggled.last(), Some(&("src".into(), false)));
        });
    }
}
