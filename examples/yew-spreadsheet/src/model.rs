use crate::{
    coordinates::{Address, Dimensions, MAX_DIMENSION, WINDOW_COLUMNS, WINDOW_ROWS},
    datasets::{self, Dataset},
    formulas::{self, Error, Expr, Value},
};
use jotai::{Atom, Store, WritableAtom, atom, derived};
use jotai_holo::{Action, Event, ion};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

pub const MAX_FILLED_CELLS: usize = 20_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub rows: usize,
    pub columns: usize,
    pub cells: BTreeMap<String, String>,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            rows: 100,
            columns: 40,
            cells: BTreeMap::new(),
        }
    }
}
impl Document {
    pub fn dimensions(&self) -> Dimensions {
        Dimensions {
            rows: self.rows,
            columns: self.columns,
        }
    }
    pub(crate) fn validate(&self) -> bool {
        self.dimensions().valid()
            && self.cells.len() <= MAX_FILLED_CELLS
            && self.cells.iter().all(|(key, value)| {
                key.parse::<Address>().is_ok_and(|a| {
                    (1..=MAX_DIMENSION).contains(&a.row) && (1..=MAX_DIMENSION).contains(&a.column)
                }) && value.chars().count() <= 512
            })
    }
}
pub fn gift_document() -> Document {
    serde_json::from_str(include_str!("../gift-budget.json")).unwrap()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    pub address: Address,
    pub draft: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    pub selected: Address,
    pub top: usize,
    pub left: usize,
    pub columns: usize,
    pub editing: Option<Edit>,
    pub focus: u64,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            selected: Address::new(1, 1),
            top: 1,
            left: 1,
            columns: WINDOW_COLUMNS,
            editing: None,
            focus: 0,
        }
    }
}
impl Editor {
    fn reveal(&mut self, dimensions: Dimensions) {
        self.selected = dimensions.clamp(self.selected);
        let rows = WINDOW_ROWS.min(dimensions.rows);
        let columns = self.columns.min(dimensions.columns);
        if self.selected.row < self.top {
            self.top = self.selected.row;
        }
        if self.selected.row >= self.top.saturating_add(rows) {
            self.top = self.selected.row - rows + 1;
        }
        if self.selected.column < self.left {
            self.left = self.selected.column;
        }
        if self.selected.column >= self.left.saturating_add(columns) {
            self.left = self.selected.column - columns + 1;
        }
        self.top = self.top.clamp(1, dimensions.rows - rows + 1);
        self.left = self.left.clamp(1, dimensions.columns - columns + 1);
    }
}

#[derive(Clone)]
pub struct CellModel {
    pub input: WritableAtom<String>,
    pub expression: Atom<Result<Expr, Error>>,
    pub value: Atom<Value>,
    /// Diagnostic for computation tests; never drives application state.
    pub evaluations: Rc<Cell<usize>>,
}
struct SheetInner {
    initial: Document,
    reset: Document,
    cells: RefCell<BTreeMap<Address, CellModel>>,
    pub document: WritableAtom<Document>,
    pub dimensions: WritableAtom<Dimensions>,
    pub issues: WritableAtom<BTreeMap<Address, Error>>,
    pub editor: WritableAtom<Editor>,
    pub notice: WritableAtom<String>,
    changed: Event<SaveMode>,
    gift: bool,
}
#[derive(Clone)]
pub struct Sheet(Rc<SheetInner>);
impl PartialEq for Sheet {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, Copy)]
pub enum SaveMode {
    Debounced,
    Immediate,
}

#[derive(Clone)]
pub enum Command {
    Select(Address),
    Begin(Address, Option<String>),
    Draft(String),
    Commit,
    Cancel,
    Move(isize, isize),
    Tab(bool),
    Go(String),
    Resize(Dimensions),
    Columns(usize),
    Reset,
}

impl Sheet {
    pub fn new(document: Document, changed: Event<SaveMode>, gift: bool) -> Self {
        let reset = if gift {
            gift_document()
        } else {
            Document::default()
        };
        Self::with_reset(document, changed, gift, reset)
    }
    fn with_reset(
        document: Document,
        changed: Event<SaveMode>,
        gift: bool,
        reset: Document,
    ) -> Self {
        let mut editor = Editor::default();
        if gift {
            editor.selected = Address::new(5, 4);
        }
        editor.reveal(document.dimensions());
        let sheet = Self(Rc::new(SheetInner {
            dimensions: atom(document.dimensions()),
            document: atom(document.clone()),
            initial: document.clone(),
            reset,
            cells: RefCell::default(),
            issues: atom(topology_issues(&document, |address| {
                formulas::parse(&document.cells[&address.to_string()])
            })),
            editor: atom(editor),
            notice: atom(String::new()),
            changed,
            gift,
        }));
        for address in document.cells.keys() {
            sheet.cell(address.parse().unwrap());
        }
        sheet
    }
    pub fn document(&self) -> &WritableAtom<Document> {
        &self.0.document
    }
    pub fn dimensions(&self) -> &WritableAtom<Dimensions> {
        &self.0.dimensions
    }
    pub fn editor(&self) -> &WritableAtom<Editor> {
        &self.0.editor
    }
    pub fn notice(&self) -> &WritableAtom<String> {
        &self.0.notice
    }
    pub fn is_gift(&self) -> bool {
        self.0.gift
    }

    // #region spreadsheet-cell
    pub fn cell(&self, address: Address) -> CellModel {
        if let Some(cell) = self.0.cells.borrow().get(&address) {
            return cell.clone();
        }
        let input = atom(
            self.0
                .initial
                .cells
                .get(&address.to_string())
                .cloned()
                .unwrap_or_default(),
        );
        let expression = ion(&input, |input| formulas::parse(&input));
        let weak = Rc::downgrade(&self.0);
        let evaluations = Rc::new(Cell::new(0));
        let value = {
            let expression = expression.clone();
            let evaluations = evaluations.clone();
            derived(move |get| {
                evaluations.set(evaluations.get() + 1);
                let Some(inner) = weak.upgrade() else {
                    return Value::Error(Error::Ref);
                };
                let dimensions = get.get(&inner.dimensions);
                if !dimensions.contains(address) {
                    return Value::Error(Error::Ref);
                }
                if let Some(error) = get.get_shared(&inner.issues).get(&address) {
                    return Value::Error(*error);
                }
                match get.get(&expression) {
                    Ok(expression) => formulas::evaluate(&expression, |reference| {
                        if !dimensions.contains(reference) {
                            return Value::Error(Error::Ref);
                        }
                        get.get(&Sheet(inner.clone()).cell(reference).value)
                    }),
                    Err(error) => Value::Error(error),
                }
            })
        };
        let cell = CellModel {
            input,
            expression,
            value,
            evaluations,
        };
        self.0.cells.borrow_mut().insert(address, cell.clone());
        cell
    }
    // #endregion spreadsheet-cell

    /// Revalidates topology before atom reads can form a reactive cycle.
    pub fn validate(&self, store: &Store) {
        let document = store.get_shared(&self.0.document);
        let issues = topology_issues(&document, |address| {
            store.get(&self.cell(address).expression)
        });
        store.set(&self.0.issues, issues);
    }

    pub fn set_cell(&self, store: &Store, address: Address, input: String) {
        if !store.get(&self.0.dimensions).contains(address) {
            return;
        }
        let cell = self.cell(address);
        let input: String = input.chars().take(512).collect();
        if store.get(&cell.input) == input {
            return;
        }
        store.batch(|| {
            store.set(&cell.input, input.clone());
            store.update(&self.0.document, |mut doc| {
                if input.is_empty() {
                    doc.cells.remove(&address.to_string());
                } else {
                    doc.cells.insert(address.to_string(), input);
                }
                doc
            });
            self.validate(store);
            store.emit(&self.0.changed, SaveMode::Debounced);
        });
    }

    fn commit(&self, store: &Store) {
        let mut editor = store.get(&self.0.editor);
        if let Some(edit) = editor.editing.take() {
            self.set_cell(store, edit.address, edit.draft);
            store.set(&self.0.editor, editor);
        }
    }
    pub fn command(&self, store: &Store, command: Command) {
        store.batch(|| self.command_inner(store, command));
    }
    fn command_inner(&self, store: &Store, command: Command) {
        if matches!(command, Command::Commit) {
            self.commit(store);
            return;
        }
        if matches!(command, Command::Reset) {
            self.reset(store);
            return;
        }
        let dimensions = store.get(&self.0.dimensions);
        let mut editor = store.get(&self.0.editor);
        let resizing = matches!(command, Command::Resize(_));
        match command {
            Command::Draft(draft) => {
                if let Some(edit) = editor.editing.as_mut() {
                    edit.draft = draft.chars().take(512).collect();
                }
            }
            Command::Cancel => {
                editor.editing = None;
                editor.focus += 1;
            }
            Command::Begin(address, replacement) => {
                if !dimensions.contains(address) {
                    return;
                }
                if editor.selected != address {
                    self.commit(store);
                }
                editor.selected = address;
                editor.editing = Some(Edit {
                    address,
                    draft: replacement.unwrap_or_else(|| store.get(&self.cell(address).input)),
                });
            }
            Command::Select(address) => {
                if editor.selected == address {
                    return;
                }
                self.commit(store);
                editor.editing = None;
                editor.selected = dimensions.clamp(address);
            }
            Command::Move(row, column) => {
                self.commit(store);
                editor.editing = None;
                editor.selected = dimensions.clamp(Address::new(
                    editor.selected.row.saturating_add_signed(row),
                    editor.selected.column.saturating_add_signed(column),
                ));
                editor.focus += 1;
            }
            Command::Tab(backward) => {
                self.commit(store);
                editor.editing = None;
                let index =
                    (editor.selected.row - 1) * dimensions.columns + editor.selected.column - 1;
                let index = if backward {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(dimensions.rows * dimensions.columns - 1)
                };
                editor.selected = Address::new(
                    index / dimensions.columns + 1,
                    index % dimensions.columns + 1,
                );
                editor.focus += 1;
            }
            Command::Go(source) => {
                let Ok(address) = source.parse::<Address>() else {
                    store.set(&self.0.notice, "Введите адрес, например AA90".into());
                    return;
                };
                if !dimensions.contains(address) {
                    store.set(&self.0.notice, "Адрес за пределами листа".into());
                    return;
                }
                self.commit(store);
                editor.editing = None;
                editor.selected = address;
                editor.focus += 1;
                store.set(&self.0.notice, String::new());
            }
            Command::Columns(columns) => {
                editor.columns = columns.clamp(1, WINDOW_COLUMNS);
            }
            Command::Resize(dimensions) => {
                self.commit(store);
                editor.editing = None;
                let dimensions = Dimensions {
                    rows: dimensions.rows.clamp(1, MAX_DIMENSION),
                    columns: dimensions.columns.clamp(1, MAX_DIMENSION),
                };
                if store.get(&self.0.dimensions) == dimensions {
                    return;
                }
                store.set(&self.0.dimensions, dimensions);
                store.update(&self.0.document, |mut doc| {
                    doc.rows = dimensions.rows;
                    doc.columns = dimensions.columns;
                    doc
                });
                self.validate(store);
            }
            Command::Commit | Command::Reset => unreachable!(),
        }
        editor.reveal(store.get(&self.0.dimensions));
        let previous = store.get(&self.0.editor);
        let persist = previous.selected != editor.selected
            || previous.top != editor.top
            || previous.left != editor.left;
        store.set(&self.0.editor, editor);
        if persist || resizing {
            store.emit(&self.0.changed, SaveMode::Debounced);
        }
    }
    fn reset(&self, store: &Store) {
        let doc = self.0.reset.clone();
        let addresses: Vec<_> = self.0.cells.borrow().keys().copied().collect();
        for address in addresses {
            store.set(
                &self.cell(address).input,
                doc.cells
                    .get(&address.to_string())
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        for (address, input) in &doc.cells {
            store.set(&self.cell(address.parse().unwrap()).input, input.clone());
        }
        store.set(&self.0.dimensions, doc.dimensions());
        store.set(&self.0.document, doc);
        let mut editor = Editor {
            columns: store.get(&self.0.editor).columns,
            ..Editor::default()
        };
        if self.0.gift {
            editor.selected = Address::new(5, 4);
        }
        editor.reveal(store.get(&self.0.dimensions));
        store.set(&self.0.editor, editor);
        self.validate(store);
        store.emit(&self.0.changed, SaveMode::Immediate);
    }
    pub fn action(&self) -> Action<Command> {
        let sheet = self.clone();
        Action::new(move |store, command| sheet.command(store, command))
    }
}

#[derive(Clone, PartialEq)]
pub struct SheetEntry {
    pub id: u64,
    pub name: String,
    pub dataset: Option<Dataset>,
    pub sheet: Sheet,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedSheet {
    pub id: u64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<Dataset>,
    pub document: Document,
    pub selected: Address,
    pub top: usize,
    pub left: usize,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u8,
    pub active: u64,
    pub sheets: Vec<SavedSheet>,
}
impl Snapshot {
    pub fn valid(&self) -> bool {
        let ids: BTreeSet<_> = self.sheets.iter().map(|sheet| sheet.id).collect();
        self.version == 1
            && !self.sheets.is_empty()
            && self.sheets.len() <= 50
            && ids.len() == self.sheets.len()
            && ids.contains(&self.active)
            && self.sheets.iter().all(|s| {
                s.id > 0 && s.id < u64::MAX && s.name.chars().count() <= 60 && s.document.validate()
            })
    }
}

#[derive(Clone, PartialEq)]
pub enum SaveStatus {
    Saved,
    Pending,
    Failed(String),
}
pub struct Book {
    pub sheets: WritableAtom<Vec<SheetEntry>>,
    pub active: WritableAtom<u64>,
    pub selected: Atom<SheetEntry>,
    pub changed: Event<SaveMode>,
    pub status: WritableAtom<SaveStatus>,
}
impl Book {
    pub fn new(store: &Store, snapshot: Option<Snapshot>) -> Self {
        let changed = Event::new();
        let mut snapshot = snapshot
            .filter(Snapshot::valid)
            .unwrap_or_else(|| Snapshot {
                version: 1,
                active: 1,
                sheets: vec![
                    SavedSheet {
                        id: 1,
                        name: "Бюджет подарков".into(),
                        dataset: None,
                        document: gift_document(),
                        selected: Address::new(5, 4),
                        top: 1,
                        left: 1,
                    },
                    SavedSheet {
                        id: 2,
                        name: "Черновик".into(),
                        dataset: None,
                        document: Document::default(),
                        selected: Address::new(1, 1),
                        top: 1,
                        left: 1,
                    },
                ],
            });
        datasets::append_missing(&mut snapshot);
        let entries: Vec<SheetEntry> = snapshot
            .sheets
            .into_iter()
            .map(|saved| {
                let sheet = if let Some(dataset) = saved.dataset {
                    Sheet::with_reset(saved.document, changed.clone(), false, dataset.document())
                } else {
                    Sheet::new(saved.document, changed.clone(), saved.id == 1)
                };
                let mut editor = Editor {
                    selected: saved.selected,
                    top: saved.top,
                    left: saved.left,
                    ..Editor::default()
                };
                editor.reveal(store.get(sheet.dimensions()));
                store.set(sheet.editor(), editor);
                sheet.validate(store);
                SheetEntry {
                    id: saved.id,
                    name: saved.name,
                    dataset: saved.dataset,
                    sheet,
                }
            })
            .collect();
        let sheets = atom(entries);
        let active = atom(snapshot.active);
        let selected = {
            let (sheets, active) = (sheets.clone(), active.clone());
            derived(move |get| {
                let id = get.get(&active);
                get.get(&sheets)
                    .into_iter()
                    .find(|entry: &SheetEntry| entry.id == id)
                    .unwrap()
            })
        };
        Self {
            sheets,
            active,
            selected,
            changed,
            status: atom(SaveStatus::Saved),
        }
    }
    pub fn switch(&self, store: &Store, id: u64) {
        if !store.get(&self.sheets).iter().any(|s| s.id == id) {
            return;
        }
        store.batch(|| {
            store
                .get(&self.selected)
                .sheet
                .command(store, Command::Commit);
            store.set(&self.active, id);
            store.emit(&self.changed, SaveMode::Immediate);
        });
    }
    pub fn add(&self, store: &Store) {
        let entries = store.get(&self.sheets);
        if entries.len() >= 50 {
            return;
        }
        let id = entries.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        if id == u64::MAX {
            return;
        }
        store.batch(|| {
            store
                .get(&self.selected)
                .sheet
                .command(store, Command::Commit);
            store.update(&self.sheets, |mut entries| {
                entries.push(SheetEntry {
                    id,
                    name: format!("Лист {id}"),
                    dataset: None,
                    sheet: Sheet::new(Document::default(), self.changed.clone(), false),
                });
                entries
            });
            store.set(&self.active, id);
            store.emit(&self.changed, SaveMode::Immediate);
        });
    }
    pub fn rename(&self, store: &Store, id: u64, name: String) {
        let name: String = name.trim().chars().take(60).collect();
        if name.is_empty() {
            return;
        }
        if !store
            .get(&self.sheets)
            .iter()
            .any(|sheet| sheet.id == id && sheet.name != name)
        {
            return;
        }
        store.update(&self.sheets, |mut entries| {
            if let Some(sheet) = entries.iter_mut().find(|s| s.id == id) {
                sheet.name = name;
            }
            entries
        });
        store.emit(&self.changed, SaveMode::Immediate);
    }
    pub fn snapshot(&self, store: &Store) -> Snapshot {
        Snapshot {
            version: 1,
            active: store.get(&self.active),
            sheets: store
                .get(&self.sheets)
                .iter()
                .map(|entry| {
                    let editor = store.get(entry.sheet.editor());
                    SavedSheet {
                        id: entry.id,
                        name: entry.name.clone(),
                        dataset: entry.dataset,
                        document: store.get(entry.sheet.document()),
                        selected: editor.selected,
                        top: editor.top,
                        left: editor.left,
                    }
                })
                .collect(),
        }
    }
}

fn topology_issues(
    document: &Document,
    mut read: impl FnMut(Address) -> Result<Expr, Error>,
) -> BTreeMap<Address, Error> {
    let dimensions = document.dimensions();
    let graph: BTreeMap<_, _> = document
        .cells
        .keys()
        .filter_map(|key| {
            let address: Address = key.parse().ok()?;
            if !dimensions.contains(address) {
                return None;
            }
            let refs = read(address).and_then(|expr| formulas::references(&expr));
            Some((address, refs))
        })
        .collect();
    fn walk(
        address: Address,
        graph: &BTreeMap<Address, Result<Vec<Address>, Error>>,
        dimensions: Dimensions,
        path: &mut BTreeSet<Address>,
        memo: &mut BTreeMap<(Address, usize), Result<(), Error>>,
    ) -> Result<(), Error> {
        if !dimensions.contains(address) {
            return Ok(());
        }
        if path.contains(&address) {
            return Err(Error::Cycle);
        }
        if path.len() >= formulas::MAX_DEPTH {
            return Err(Error::Limit);
        }
        let key = (address, path.len());
        if let Some(result) = memo.get(&key) {
            return *result;
        }
        let Some(references) = graph.get(&address) else {
            return Ok(());
        };
        let references = references.as_ref().map_err(|e| *e)?;
        path.insert(address);
        let result = references
            .iter()
            .try_for_each(|reference| walk(*reference, graph, dimensions, path, memo));
        path.remove(&address);
        memo.insert(key, result);
        result
    }
    graph
        .keys()
        .filter_map(|address| {
            walk(
                *address,
                &graph,
                dimensions,
                &mut BTreeSet::new(),
                &mut BTreeMap::new(),
            )
            .err()
            .map(|error| (*address, error))
        })
        .collect()
}
