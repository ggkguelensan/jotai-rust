use jotai::Store;
use jotai_holo::Event;
use std::{cell::Cell, rc::Rc};
use yew_spreadsheet::{
    coordinates::{Address, Dimensions},
    formulas::{self, Error, Value},
    model::{Book, Command, Document, Sheet, gift_document},
};

fn address(source: &str) -> Address {
    source.parse().unwrap()
}
fn value(sheet: &Sheet, store: &Store, source: &str) -> Value {
    store.get(&sheet.cell(address(source)).value)
}
fn edit(sheet: &Sheet, store: &Store, source: &str, input: &str) {
    sheet.set_cell(store, address(source), input.into());
}

#[test]
fn gift_budget_recalculates_shared_intermediates_once() {
    let store = Store::new();
    let sheet = Sheet::new(gift_document(), Event::new(), true);
    assert_eq!(value(&sheet, &store, "D5"), Value::Number(4136.0));
    let product = sheet.cell(address("D1"));
    let before = product.evaluations.get();
    let total = sheet.cell(address("D5"));
    let _subscription = store.subscribe(&total.value, || {});
    edit(&sheet, &store, "B1", "20");
    assert_eq!(value(&sheet, &store, "D5"), Value::Number(4840.0));
    assert_eq!(product.evaluations.get(), before + 1);
}

#[test]
fn formula_subset_and_errors() {
    let read = |addr| {
        if addr == address("A1") {
            Value::Number(3.0)
        } else {
            Value::Text("name".into())
        }
    };
    for (input, expected) in [
        ("=-(2+3)*4/2", Value::Number(-10.0)),
        ("=sum(A1:B1,2,1e1)", Value::Number(15.0)),
        ("=1/0", Value::Error(Error::DivideByZero)),
        ("=B1+1", Value::Error(Error::Value)),
        ("=1e308*10", Value::Error(Error::Number)),
        ("=SUM(A1:ZZ1000)", Value::Error(Error::Limit)),
    ] {
        assert_eq!(
            formulas::evaluate(&formulas::parse(input).unwrap(), read),
            expected,
            "{input}"
        );
    }
    assert!(formulas::parse("=SUM(").is_err());
    assert!(formulas::parse("=alert('hello')").is_err());
    assert!(formulas::parse("=1;2").is_err());
}

#[test]
fn cycles_are_values_and_repair_restores_dependents() {
    let store = Store::new();
    let sheet = Sheet::new(Document::default(), Event::new(), false);
    edit(&sheet, &store, "A1", "=B1");
    edit(&sheet, &store, "B1", "=A1");
    edit(&sheet, &store, "C1", "=A1+1");
    assert_eq!(value(&sheet, &store, "A1"), Value::Error(Error::Cycle));
    assert_eq!(value(&sheet, &store, "C1"), Value::Error(Error::Cycle));
    edit(&sheet, &store, "B1", "5");
    assert_eq!(value(&sheet, &store, "C1"), Value::Number(6.0));
}

#[test]
fn formula_replacement_unsubscribes_old_dependency() {
    let store = Store::new();
    let sheet = Sheet::new(Document::default(), Event::new(), false);
    edit(&sheet, &store, "A1", "1");
    edit(&sheet, &store, "B1", "2");
    edit(&sheet, &store, "C1", "=A1");
    let cell = sheet.cell(address("C1"));
    let _sub = store.subscribe(&cell.value, || {});
    edit(&sheet, &store, "C1", "=B1");
    let calls = cell.evaluations.get();
    edit(&sheet, &store, "A1", "20");
    assert_eq!(cell.evaluations.get(), calls);
    assert_eq!(value(&sheet, &store, "C1"), Value::Number(2.0));
}

#[test]
fn offscreen_cells_survive_resize_and_sheet_graph_is_store_local() {
    let store = Store::new();
    let other = Store::new();
    let sheet = Sheet::new(Document::default(), Event::new(), false);
    edit(&sheet, &store, "AA90", "7");
    edit(&sheet, &store, "A1", "=AA90*3");
    assert_eq!(value(&sheet, &store, "A1"), Value::Number(21.0));
    sheet.command(
        &store,
        Command::Resize(Dimensions {
            rows: 50,
            columns: 40,
        }),
    );
    assert_eq!(value(&sheet, &store, "A1"), Value::Error(Error::Ref));
    sheet.command(
        &store,
        Command::Resize(Dimensions {
            rows: 100,
            columns: 40,
        }),
    );
    assert_eq!(value(&sheet, &store, "A1"), Value::Number(21.0));
    assert_eq!(value(&sheet, &other, "AA90"), Value::Blank);
}

#[test]
fn editing_cancel_tab_and_camera() {
    let store = Store::new();
    let sheet = Sheet::new(gift_document(), Event::new(), true);
    sheet.command(&store, Command::Begin(address("B1"), Some("20".into())));
    sheet.command(&store, Command::Cancel);
    assert_eq!(value(&sheet, &store, "B1"), Value::Number(12.0));
    sheet.command(&store, Command::Go("AN1".into()));
    sheet.command(&store, Command::Begin(address("AN1"), Some("5".into())));
    sheet.command(&store, Command::Tab(false));
    assert_eq!(store.get(sheet.editor()).selected, address("A2"));
    assert_eq!(value(&sheet, &store, "AN1"), Value::Number(5.0));
    sheet.command(&store, Command::Go("AA90".into()));
    let editor = store.get(sheet.editor());
    assert_eq!(editor.top, 83);
    assert_eq!(editor.left, 24);
}

#[test]
fn no_op_edit_does_not_emit_or_invalidate_document() {
    let store = Store::new();
    let changed = Event::new();
    let calls = Rc::new(Cell::new(0));
    let _sub = {
        let calls = calls.clone();
        store.listen(&changed, move |_| calls.set(calls.get() + 1))
    };
    let sheet = Sheet::new(gift_document(), changed, true);
    let before = store.get_shared(sheet.document());
    edit(&sheet, &store, "B1", "12");
    assert_eq!(calls.get(), 0);
    assert!(Rc::ptr_eq(&before, &store.get_shared(sheet.document())));
}

#[test]
fn workbook_switch_commits_draft_and_snapshot_restores_names_cells_and_camera() {
    let store = Store::new();
    let book = Book::new(&store, None);
    let gifts = store.get(&book.selected).sheet;
    gifts.command(&store, Command::Begin(address("B1"), Some("20".into())));
    book.switch(&store, 2);
    assert_eq!(value(&gifts, &store, "B1"), Value::Number(20.0));
    let scratch = store.get(&book.selected).sheet;
    scratch.command(&store, Command::Go("AA90".into()));
    edit(&scratch, &store, "AA90", "42");
    book.rename(&store, 2, "Заметки".into());
    let snapshot = book.snapshot(&store);
    assert!(snapshot.valid());
    let encoded = serde_json::to_string(&snapshot).unwrap();
    let other = Store::new();
    let restored = Book::new(&other, Some(serde_json::from_str(&encoded).unwrap()));
    let selected = other.get(&restored.selected);
    assert_eq!(selected.name, "Заметки");
    assert_eq!(other.get(selected.sheet.editor()).selected, address("AA90"));
    assert_eq!(value(&selected.sheet, &other, "AA90"), Value::Number(42.0));
    assert_eq!(
        value(&other.get(&restored.sheets)[0].sheet, &other, "B1"),
        Value::Number(20.0)
    );
}

#[test]
fn bounded_reference_depth_does_not_panic() {
    let store = Store::new();
    let sheet = Sheet::new(Document::default(), Event::new(), false);
    sheet.command(
        &store,
        Command::Resize(Dimensions {
            rows: 200,
            columns: 40,
        }),
    );
    for row in (1..=129).rev() {
        edit(
            &sheet,
            &store,
            &format!("A{row}"),
            &if row == 129 {
                "1".to_string()
            } else {
                format!("=A{}", row + 1)
            },
        );
    }
    assert_eq!(value(&sheet, &store, "A1"), Value::Error(Error::Limit));
    assert_eq!(value(&sheet, &store, "A129"), Value::Number(1.0));
}

#[test]
fn coordinate_roundtrip_and_invalid_addresses() {
    for source in ["A1", "Z99", "AA90", "ALL1000"] {
        assert_eq!(address(source).to_string(), source);
    }
    for source in ["A0", "1", "A-1", "AA", "A1.2", "A18446744073709551616"] {
        assert!(source.parse::<Address>().is_err());
    }
}

#[test]
fn initial_cyclic_document_is_safe_before_any_edit() {
    let mut document = Document::default();
    document.cells.insert("A1".into(), "=B1".into());
    document.cells.insert("B1".into(), "=A1".into());
    let sheet = Sheet::new(document, Event::new(), false);
    assert_eq!(
        value(&sheet, &Store::new(), "A1"),
        Value::Error(Error::Cycle)
    );
}
