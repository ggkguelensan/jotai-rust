use jotai::Store;
use yew_spreadsheet::{
    coordinates::Address,
    datasets::{Dataset, parse_csv},
    model::{Book, Command, Snapshot},
};

#[test]
fn full_datasets_preserve_headers_last_rows_precision_and_missing_fields() {
    let penguins = Dataset::Penguins.document();
    assert_eq!((penguins.rows, penguins.columns), (345, 8));
    assert_eq!(penguins.cells["A1"], "species");
    assert_eq!(penguins.cells["C2"], "39.1");
    assert_eq!(penguins.cells["H345"], "2009");
    let titanic = Dataset::Titanic.document();
    assert_eq!((titanic.rows, titanic.columns), (892, 15));
    assert_eq!(titanic.cells["A1"], "survived");
    assert_eq!(titanic.cells["G3"], "71.2833");
    assert!(!titanic.cells.contains_key("L2"));
    assert_eq!(titanic.cells["O892"], "True");
}

#[test]
fn csv_handles_bom_quoted_commas_newlines_and_empty_fields() {
    let doc = parse_csv("\u{feff}\"a,b\",value,empty\r\n\"two\nlines\",1,\r\n").unwrap();
    assert_eq!((doc.rows, doc.columns), (2, 3));
    assert_eq!(doc.cells["A1"], "a,b");
    assert_eq!(doc.cells["A2"], "two\nlines");
    assert!(!doc.cells.contains_key("C2"));
}

#[test]
fn csv_rejects_unsupported_sizes_and_ragged_records_before_sheet_creation() {
    assert!(parse_csv("").is_err());
    assert!(parse_csv("a,b\n1\n").is_err());
    assert!(parse_csv(&"1\n".repeat(1001)).is_err());
    assert!(parse_csv(&vec!["a"; 1001].join(",")).is_err());
    assert!(parse_csv(&"a".repeat(513)).is_err());
    let row = format!("{}\n", vec!["1"; 22].join(","));
    assert!(parse_csv(&row.repeat(1000)).is_err());
}

#[test]
fn old_snapshots_gain_datasets_without_losing_existing_cells_ids_or_selection() {
    let store = Store::new();
    let book = Book::new(&store, None);
    book.add(&store);
    let custom_id = store.get(&book.active);
    let custom = store.get(&book.selected).sheet;
    custom.set_cell(&store, Address::new(1, 1), "Существующая работа".into());
    let mut snapshot = book.snapshot(&store);
    snapshot.sheets.retain(|sheet| sheet.dataset.is_none());
    let legacy_json = serde_json::to_string(&snapshot).unwrap();
    assert!(!legacy_json.contains("\"dataset\""));
    let other = Store::new();
    let restored = Book::new(&other, Some(serde_json::from_str(&legacy_json).unwrap()));
    assert_eq!(other.get(&restored.active), custom_id);
    let selected = other.get(&restored.selected);
    assert_eq!(
        other.get(selected.sheet.document()).cells["A1"],
        "Существующая работа"
    );
    let entries = other.get(&restored.sheets);
    assert_eq!(entries.len(), 5);
    assert_eq!(
        entries
            .iter()
            .filter(|sheet| sheet.dataset.is_some())
            .count(),
        2
    );
    assert!(
        entries
            .iter()
            .filter(|sheet| sheet.dataset.is_some())
            .all(|sheet| sheet.id > custom_id)
    );
}

#[test]
fn restoration_keeps_dataset_edits_renames_and_identity_while_reset_restores_csv() {
    let store = Store::new();
    let book = Book::new(&store, None);
    let penguins = store
        .get(&book.sheets)
        .into_iter()
        .find(|sheet| sheet.dataset == Some(Dataset::Penguins))
        .unwrap();
    penguins
        .sheet
        .set_cell(&store, Address::new(2, 3), "55".into());
    book.rename(&store, penguins.id, "Пингвины".into());
    book.switch(&store, penguins.id);
    let snapshot = book.snapshot(&store);
    assert!(snapshot.valid());
    let json = serde_json::to_string(&snapshot).unwrap();
    let other = Store::new();
    let restored = Book::new(
        &other,
        Some(serde_json::from_str::<Snapshot>(&json).unwrap()),
    );
    assert_eq!(other.get(&restored.sheets).len(), 4);
    let selected = other.get(&restored.selected);
    assert_eq!(selected.name, "Пингвины");
    assert_eq!(selected.id, penguins.id);
    assert_eq!(other.get(selected.sheet.document()).cells["C2"], "55");
    selected.sheet.command(&other, Command::Reset);
    assert_eq!(
        other.get(selected.sheet.document()),
        Dataset::Penguins.document()
    );
}
