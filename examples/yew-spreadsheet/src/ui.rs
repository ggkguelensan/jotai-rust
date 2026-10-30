use gloo_events::EventListener;
use gloo_timers::callback::Timeout;
use jotai::Store;
use jotai_holo::ion;
use jotai_holo_yew::{Holo, use_holo};
use std::{cell::RefCell, rc::Rc};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_spreadsheet::{
    coordinates::{Address, Dimensions, WINDOW_ROWS, column_name},
    formulas::Value,
    model::{Book, Command, SaveMode, SaveStatus, Sheet, SheetEntry, Snapshot},
};

const STORAGE_KEY: &str = "jotai-rust/spreadsheet/v1";

#[derive(Clone)]
struct Session {
    store: Store,
    book: Rc<Book>,
}
impl PartialEq for Session {
    fn eq(&self, other: &Self) -> bool {
        self.store == other.store
    }
}
impl Session {
    fn load() -> Self {
        let store = Store::new();
        let restored = (|| -> Result<Option<Snapshot>, String> {
            let storage = web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .ok_or("Локальное хранилище недоступно")?;
            let Some(text) = storage
                .get_item(STORAGE_KEY)
                .map_err(|_| "Не удалось прочитать сохранение")?
            else {
                return Ok(None);
            };
            let snapshot: Snapshot =
                serde_json::from_str(&text).map_err(|_| "Сохранение повреждено")?;
            if !snapshot.valid() {
                return Err("Неподдерживаемый формат сохранения".into());
            }
            Ok(Some(snapshot))
        })();
        let book = Rc::new(Book::new(&store, restored.clone().ok().flatten()));
        if let Err(error) = restored {
            store.set(&book.status, SaveStatus::Failed(error));
        }
        Self { store, book }
    }
    fn save(&self) {
        let result = (|| -> Result<(), String> {
            let storage = web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .ok_or("Локальное хранилище недоступно")?;
            let text = serde_json::to_string(&self.book.snapshot(&self.store))
                .map_err(|_| "Не удалось подготовить сохранение")?;
            storage
                .set_item(STORAGE_KEY, &text)
                .map_err(|_| "Не удалось сохранить: проверьте место и доступ к хранилищу".into())
        })();
        self.store.set(
            &self.book.status,
            match result {
                Ok(()) => SaveStatus::Saved,
                Err(error) => SaveStatus::Failed(error),
            },
        );
    }
}

#[component]
pub fn App() -> Html {
    let session = use_state(|| None::<Session>);
    use_effect_with((), {
        let session = session.clone();
        move |_| {
            session.set(Some(Session::load()));
            || {}
        }
    });
    match &*session {
        Some(session) => html! { <Workbook session={session.clone()} /> },
        None => html! { <main class="loading" role="status">{"Открываем книгу…"}</main> },
    }
}

#[derive(Clone, PartialEq, Properties)]
struct WorkbookProps {
    session: Session,
}
#[derive(Clone, PartialEq)]
struct BookValues {
    active: Option<SheetEntry>,
    sheets: Vec<SheetEntry>,
}

#[component]
fn Workbook(props: &WorkbookProps) -> Html {
    let session = props.session.clone();
    let binding = use_memo(session.clone(), |session| {
        Holo::new(|| BookValues {
            active: None,
            sheets: vec![],
        })
        .read(&session.book.selected, |props, entry| {
            props.active = Some(entry)
        })
        .read(&session.book.sheets, |props, sheets| props.sheets = sheets)
    });
    let values = use_holo((*binding).clone(), session.store.clone());
    // One owner for the persistence subscription. Pending edits flush on cleanup/pagehide.
    use_effect_with(session.clone(), |session| {
        let timer: Rc<RefCell<Option<Timeout>>> = Rc::default();
        let weak_store = session.store.downgrade();
        let weak_book = Rc::downgrade(&session.book);
        let pending = timer.clone();
        let subscription = session.store.listen(&session.book.changed, move |mode| {
            pending.borrow_mut().take();
            let (Some(store), Some(book)) = (weak_store.upgrade(), weak_book.upgrade()) else {
                return;
            };
            let session = Session { store, book };
            session.store.set(&session.book.status, SaveStatus::Pending);
            match mode {
                SaveMode::Immediate => session.save(),
                SaveMode::Debounced => {
                    *pending.borrow_mut() = Some(Timeout::new(250, move || session.save()));
                }
            }
        });
        let onhide = {
            let session = session.clone();
            EventListener::new(&web_sys::window().unwrap(), "pagehide", move |_| {
                if session.store.get(&session.book.status) == SaveStatus::Pending {
                    session.save();
                }
            })
        };
        let session = session.clone();
        move || {
            drop(subscription);
            drop(onhide);
            timer.borrow_mut().take();
            if session.store.get(&session.book.status) == SaveStatus::Pending {
                session.save();
            }
        }
    });
    let add = {
        let session = session.clone();
        Callback::from(move |_| session.book.add(&session.store))
    };
    let active = values.active.unwrap();
    html! {
        <main class="workbook">
            <header class="book-heading">
                <div><div class="eyebrow">{"РАБОЧАЯ КНИГА"}</div><h1>{"Бюджет подарков"}</h1></div>
                <SaveIndicator session={session.clone()} />
            </header>
            <p class="intro">{"Меняйте значения и формулы — связанные ячейки пересчитаются."}</p>
            <div class="sheet-surface">
                <SheetWorkspace key={active.id} sheet={active.sheet.clone()} store={session.store.clone()} />
                <nav class="sheet-tabs" aria-label="Листы книги">
                    {for values.sheets.iter().map(|entry| html! {
                        <SheetTab key={entry.id} entry={entry.clone()} active={entry.id == active.id} session={session.clone()} />
                    })}
                    <button class="add-sheet" aria-label="Добавить лист" onclick={add} disabled={values.sheets.len() >= 50}>{"+"}</button>
                </nav>
            </div>
            <footer class="help">
                <span><kbd>{"Enter"}</kbd>{" / двойной клик — редактировать"}</span>
                <span><kbd>{"Esc"}</kbd>{" — отменить"}</span>
                <span><kbd>{"Tab"}</kbd>{" / стрелки — перейти"}</span>
            </footer>
        </main>
    }
}

#[component]
fn SaveIndicator(props: &WorkbookProps) -> Html {
    let binding = use_memo(props.session.clone(), |session| {
        Holo::new(|| SaveStatus::Saved).read(&session.book.status, |props, value| *props = value)
    });
    let status = use_holo((*binding).clone(), props.session.store.clone());
    let retry = {
        let session = props.session.clone();
        Callback::from(move |_| session.save())
    };
    match status {
        SaveStatus::Saved => {
            html! { <span class="save-status saved" role="status">{"✓ Сохранено локально"}</span> }
        }
        SaveStatus::Pending => {
            html! { <span class="save-status" role="status">{"Сохраняем…"}</span> }
        }
        SaveStatus::Failed(message) => {
            html! { <span class="save-status failed" role="alert">{message}<button onclick={retry}>{"Повторить"}</button></span> }
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
struct TabProps {
    entry: SheetEntry,
    active: bool,
    session: Session,
}
#[component]
fn SheetTab(props: &TabProps) -> Html {
    let editing = use_state(|| None::<String>);
    let input_ref = use_node_ref();
    use_effect_with(editing.is_some(), {
        let input_ref = input_ref.clone();
        move |editing| {
            if *editing && let Some(input) = input_ref.cast::<HtmlInputElement>() {
                let _ = input.focus();
                input.select();
            }
            || {}
        }
    });
    let switch = {
        let session = props.session.clone();
        let id = props.entry.id;
        Callback::from(move |_| session.book.switch(&session.store, id))
    };
    let rename = {
        let editing = editing.clone();
        let name = props.entry.name.clone();
        Callback::from(move |_| editing.set(Some(name.clone())))
    };
    if let Some(name) = &*editing {
        let oninput = {
            let editing = editing.clone();
            Callback::from(move |event: InputEvent| {
                editing.set(Some(
                    event.target_unchecked_into::<HtmlInputElement>().value(),
                ))
            })
        };
        let onkeydown = {
            let (editing, session, id) = (editing.clone(), props.session.clone(), props.entry.id);
            Callback::from(move |event: KeyboardEvent| {
                if event.key() == "Enter" {
                    event.prevent_default();
                    if let Some(name) = &*editing {
                        session.book.rename(&session.store, id, name.clone());
                    }
                    editing.set(None);
                } else if event.key() == "Escape" {
                    editing.set(None);
                }
            })
        };
        let onblur = {
            let editing = editing.clone();
            let session = props.session.clone();
            let id = props.entry.id;
            Callback::from(move |_| {
                if let Some(name) = &*editing {
                    session.book.rename(&session.store, id, name.clone());
                }
                editing.set(None);
            })
        };
        html! { <input ref={input_ref} class="tab-rename" aria-label="Название листа" value={name.clone()} maxlength="60" {oninput} {onkeydown} {onblur} /> }
    } else {
        html! { <button class={classes!("sheet-tab", props.active.then_some("active"))} data-sheet-id={props.entry.id.to_string()}
        aria-current={props.active.then_some("page")} onclick={switch} ondblclick={rename} title="Двойной клик — переименовать">{&props.entry.name}</button> }
    }
}

#[derive(Clone, PartialEq, Properties)]
struct SheetProps {
    sheet: Sheet,
    store: Store,
}

#[component]
fn SheetWorkspace(props: &SheetProps) -> Html {
    let columns = use_state(|| if window_width() < 600.0 { 2 } else { 4 });
    use_effect_with((), {
        let columns = columns.clone();
        move |_| {
            let listener = EventListener::new(&web_sys::window().unwrap(), "resize", move |_| {
                columns.set(if window_width() < 600.0 { 2 } else { 4 })
            });
            move || drop(listener)
        }
    });
    use_effect_with(
        (props.sheet.clone(), props.store.clone(), *columns),
        |(sheet, store, columns)| {
            sheet.command(store, Command::Columns(*columns));
            || {}
        },
    );
    html! {
        <>
            <SheetToolbar sheet={props.sheet.clone()} store={props.store.clone()} />
            <FormulaBar sheet={props.sheet.clone()} store={props.store.clone()} />
            <SheetGrid sheet={props.sheet.clone()} store={props.store.clone()} />
            <SheetSummary sheet={props.sheet.clone()} store={props.store.clone()} />
        </>
    }
}
fn window_width() -> f64 {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(800.0)
}

#[derive(Clone, PartialEq)]
struct ToolbarValues {
    dimensions: Dimensions,
    notice: String,
}
#[component]
fn SheetToolbar(props: &SheetProps) -> Html {
    let go = use_state(String::new);
    let binding = use_memo(props.clone(), |props| {
        Holo::new(|| ToolbarValues {
            dimensions: Dimensions::default(),
            notice: String::new(),
        })
        .read(props.sheet.dimensions(), |props, value| {
            props.dimensions = value
        })
        .read(props.sheet.notice(), |props, value| props.notice = value)
    });
    let values = use_holo((*binding).clone(), props.store.clone());
    let ongo = {
        let (sheet, store, go) = (props.sheet.clone(), props.store.clone(), go.clone());
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            sheet.command(&store, Command::Go((*go).clone()));
        })
    };
    let oninput = {
        let go = go.clone();
        Callback::from(move |event: InputEvent| {
            go.set(event.target_unchecked_into::<HtmlInputElement>().value())
        })
    };
    let rows = {
        let (sheet, store) = (props.sheet.clone(), props.store.clone());
        Callback::from(move |event: Event| {
            if let Ok(rows) = event
                .target_unchecked_into::<HtmlInputElement>()
                .value()
                .parse()
            {
                sheet.command(
                    &store,
                    Command::Resize(Dimensions {
                        rows,
                        columns: store.get(sheet.dimensions()).columns,
                    }),
                );
            }
        })
    };
    let columns = {
        let (sheet, store) = (props.sheet.clone(), props.store.clone());
        Callback::from(move |event: Event| {
            if let Ok(columns) = event
                .target_unchecked_into::<HtmlInputElement>()
                .value()
                .parse()
            {
                sheet.command(
                    &store,
                    Command::Resize(Dimensions {
                        columns,
                        rows: store.get(sheet.dimensions()).rows,
                    }),
                );
            }
        })
    };
    let reset = {
        let (sheet, store) = (props.sheet.clone(), props.store.clone());
        Callback::from(move |_| sheet.command(&store, Command::Reset))
    };
    html! {
        <div class="toolbar">
            <form class="go-form" onsubmit={ongo}><input aria-label="Перейти к ячейке" placeholder="AA90" value={(*go).clone()} {oninput} /><button type="submit">{"Перейти"}</button></form>
            <div class="dimensions"><label>{"Строки"}<input aria-label="Число строк" type="number" min="1" max="1000" value={values.dimensions.rows.to_string()} onchange={rows} /></label>
                <label>{"Столбцы"}<input aria-label="Число столбцов" type="number" min="1" max="1000" value={values.dimensions.columns.to_string()} onchange={columns} /></label></div>
            <button class="reset-sheet" onclick={reset}>{"Сбросить лист"}</button>
            if !values.notice.is_empty() { <span class="notice" role="alert">{values.notice}</span> }
        </div>
    }
}

#[derive(Clone, PartialEq)]
struct FormulaValues {
    address: Address,
    input: String,
    value: Value,
    editing: bool,
}
#[component]
fn FormulaBar(props: &SheetProps) -> Html {
    let source = use_memo(props.clone(), |props| {
        let sheet = props.sheet.clone();
        jotai::derived(move |get| {
            let editor = get.get(sheet.editor());
            let cell = sheet.cell(editor.selected);
            FormulaValues {
                address: editor.selected,
                input: editor
                    .editing
                    .as_ref()
                    .map(|edit| edit.draft.clone())
                    .unwrap_or_else(|| get.get(&cell.input)),
                value: get.get(&cell.value),
                editing: editor.editing.is_some(),
            }
        })
    });
    let binding = use_memo(source.clone(), |source| {
        Holo::new(|| FormulaValues {
            address: Address::new(1, 1),
            input: String::new(),
            value: Value::Blank,
            editing: false,
        })
        .read(source, |props, value| *props = value)
    });
    let values = use_holo((*binding).clone(), props.store.clone());
    let onfocus = {
        let (sheet, store) = (props.sheet.clone(), props.store.clone());
        Callback::from(move |_| {
            let editor = store.get(sheet.editor());
            if editor.editing.is_none() {
                sheet.command(&store, Command::Begin(editor.selected, None));
            }
        })
    };
    let oninput = {
        let (sheet, store) = (props.sheet.clone(), props.store.clone());
        Callback::from(move |event: InputEvent| {
            sheet.command(
                &store,
                Command::Draft(event.target_unchecked_into::<HtmlInputElement>().value()),
            )
        })
    };
    let onkeydown = {
        let (sheet, store) = (props.sheet.clone(), props.store.clone());
        Callback::from(move |event: KeyboardEvent| match event.key().as_str() {
            "Enter" => {
                event.prevent_default();
                sheet.command(&store, Command::Move(1, 0));
            }
            "Escape" => {
                event.prevent_default();
                sheet.command(&store, Command::Cancel);
            }
            _ => {}
        })
    };
    let onblur = {
        let (sheet, store) = (props.sheet.clone(), props.store.clone());
        Callback::from(move |_| sheet.command(&store, Command::Commit))
    };
    html! {
        <div class="formula-bar">
            <span class="address-box">{values.address.to_string()}</span><span class="fx" aria-hidden="true">{"ƒx"}</span>
            <input aria-label="Формула" class="formula-input" placeholder="Значение или =SUM(A1:A5)" value={values.input} maxlength="512" spellcheck="false" {onfocus} {oninput} {onkeydown} {onblur} />
            <span class="formula-result" title="Вычисленное значение">{values.value.display()}</span>
        </div>
    }
}

#[derive(Clone, PartialEq)]
struct Camera {
    top: usize,
    left: usize,
    rows: usize,
    columns: usize,
}
#[component]
fn SheetGrid(props: &SheetProps) -> Html {
    let camera = use_memo(props.clone(), |props| {
        let sheet = props.sheet.clone();
        jotai::derived(move |get| {
            let editor = get.get(sheet.editor());
            let dimensions = get.get(sheet.dimensions());
            Camera {
                top: editor.top,
                left: editor.left,
                rows: WINDOW_ROWS.min(dimensions.rows),
                columns: editor.columns.min(dimensions.columns),
            }
        })
    });
    let binding = use_memo(camera.clone(), |camera| {
        Holo::new(|| Camera {
            top: 1,
            left: 1,
            rows: 8,
            columns: 4,
        })
        .read(camera, |props, value| *props = value)
    });
    let camera = use_holo((*binding).clone(), props.store.clone());
    html! {
        <div class="grid-wrapper">
            <table class="sheet-grid" aria-label="Электронная таблица">
                <thead><tr><th class="corner" aria-label="Номера строк"></th>{for (0..camera.columns).map(|column| html! { <th scope="col">{column_name(camera.left + column)}</th> })}</tr></thead>
                <tbody>{for (0..camera.rows).map(|row| html! {
                    <tr key={row}><th scope="row">{camera.top + row}</th>{for (0..camera.columns).map(|column| html! {
                        <td key={column}><ConnectedCell address={Address::new(camera.top + row,camera.left + column)} sheet={props.sheet.clone()} store={props.store.clone()} /></td>
                    })}</tr>
                })}</tbody>
            </table>
        </div>
    }
}

#[derive(Clone, PartialEq)]
struct CellEditor {
    selected: bool,
    editing: bool,
    draft: String,
    focus: u64,
}

#[derive(Clone, PartialEq, Properties)]
struct ConnectedCellProps {
    address: Address,
    sheet: Sheet,
    store: Store,
}
// #region spreadsheet-binding
#[component]
fn ConnectedCell(props: &ConnectedCellProps) -> Html {
    let binding = use_memo(props.clone(), |props| {
        let address = props.address;
        let cell = props.sheet.cell(address);
        let editor = ion(props.sheet.editor(), move |editor| {
            let selected = editor.selected == address;
            let edit = editor
                .editing
                .as_ref()
                .filter(|edit| edit.address == address);
            CellEditor {
                selected,
                editing: edit.is_some(),
                draft: edit.map(|edit| edit.draft.clone()).unwrap_or_default(),
                focus: if selected { editor.focus } else { 0 },
            }
        });
        let action = props.sheet.action();
        Holo::new(move || CellProps {
            address,
            input: String::new(),
            value: Value::Blank,
            editor: CellEditor {
                selected: false,
                editing: false,
                draft: String::new(),
                focus: 0,
            },
            command: Callback::noop(),
        })
        .read(&cell.input, |props, value| props.input = value)
        .read(&cell.value, |props, value| props.value = value)
        .read(&editor, |props, value| props.editor = value)
        .write(&action, |props, callback| props.command = callback)
    });
    let values = use_holo((*binding).clone(), props.store.clone());
    html! { <CellView ..values /> }
}
// #endregion spreadsheet-binding

#[derive(Clone, PartialEq, Properties)]
struct CellProps {
    address: Address,
    input: String,
    value: Value,
    editor: CellEditor,
    command: Callback<Command>,
}
#[component]
fn CellView(props: &CellProps) -> Html {
    let input_ref = use_node_ref();
    use_effect_with(
        (props.editor.selected, props.editor.focus, props.address),
        {
            let input_ref = input_ref.clone();
            move |(selected, focus, _)| {
                if *selected
                    && *focus > 0
                    && let Some(input) = input_ref.cast::<HtmlInputElement>()
                {
                    let options = web_sys::FocusOptions::new();
                    options.set_prevent_scroll(true);
                    let _ = input.focus_with_options(&options);
                }
                || {}
            }
        },
    );
    let address = props.address;
    let onfocus = {
        let command = props.command.clone();
        Callback::from(move |_| command.emit(Command::Select(address)))
    };
    let ondblclick = {
        let command = props.command.clone();
        Callback::from(move |_| command.emit(Command::Begin(address, None)))
    };
    let oninput = {
        let command = props.command.clone();
        Callback::from(move |event: InputEvent| {
            command.emit(Command::Draft(
                event.target_unchecked_into::<HtmlInputElement>().value(),
            ))
        })
    };
    let onblur = {
        let command = props.command.clone();
        Callback::from(move |_| command.emit(Command::Commit))
    };
    let onkeydown = {
        let command = props.command.clone();
        let editing = props.editor.editing;
        Callback::from(move |event: KeyboardEvent| {
            if event.is_composing() {
                return;
            }
            let key = event.key();
            let next = match key.as_str() {
                "Escape" => Some(Command::Cancel),
                "Enter" if editing => {
                    Some(Command::Move(if event.shift_key() { -1 } else { 1 }, 0))
                }
                "Enter" | "F2" => Some(Command::Begin(address, None)),
                "Tab" => Some(Command::Tab(event.shift_key())),
                "ArrowDown" if !editing => Some(Command::Move(1, 0)),
                "ArrowUp" if !editing => Some(Command::Move(-1, 0)),
                "ArrowRight" if !editing => Some(Command::Move(0, 1)),
                "ArrowLeft" if !editing => Some(Command::Move(0, -1)),
                "Backspace" | "Delete" if !editing => {
                    Some(Command::Begin(address, Some(String::new())))
                }
                _ if !editing
                    && key.chars().count() == 1
                    && !event.ctrl_key()
                    && !event.meta_key()
                    && !event.alt_key() =>
                {
                    Some(Command::Begin(address, Some(key)))
                }
                _ => None,
            };
            if let Some(next) = next {
                event.prevent_default();
                command.emit(next);
            }
        })
    };
    html! { <input ref={input_ref} class={classes!("sheet-cell",props.editor.selected.then_some("selected"),props.editor.editing.then_some("editing"))}
    aria-label={address.to_string()} data-address={address.to_string()} data-selected={props.editor.selected.to_string()}
    data-formula={props.input.trim().starts_with('=').to_string()} data-numeric={matches!(props.value,Value::Number(_)).to_string()}
    aria-invalid={matches!(props.value,Value::Error(_)).to_string()} readonly={!props.editor.editing} autocomplete="off" spellcheck="false" maxlength="512"
    title={props.input.clone()} value={if props.editor.editing {props.editor.draft.clone()} else {props.value.display()}}
    {onfocus} {ondblclick} {oninput} {onblur} {onkeydown} /> }
}

#[derive(Clone, PartialEq)]
struct Summary {
    total: Value,
    top: usize,
    left: usize,
}
#[component]
fn SheetSummary(props: &SheetProps) -> Html {
    let binding = use_memo(props.clone(), |props| {
        let camera = ion(props.sheet.editor(), |editor| (editor.top, editor.left));
        Holo::new(|| Summary {
            total: Value::Blank,
            top: 1,
            left: 1,
        })
        .read(
            &props.sheet.cell(Address::new(5, 4)).value,
            |props, value| props.total = value,
        )
        .read(&camera, |props, (top, left)| {
            props.top = top;
            props.left = left;
        })
    });
    let summary = use_holo((*binding).clone(), props.store.clone());
    html! { <div class="sheet-summary"><span>{format!("Окно от {}{}",column_name(summary.left),summary.top)}</span><span>{"D5 · Итого "}<strong class="bundle-total">{summary.total.display()}</strong></span></div> }
}
