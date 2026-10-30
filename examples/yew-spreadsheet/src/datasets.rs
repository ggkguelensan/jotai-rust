//! Bundled CSV samples and bounded, all-or-nothing document construction.
use crate::{
    coordinates::{Address, MAX_DIMENSION},
    model::{Document, MAX_FILLED_CELLS, SavedSheet, Snapshot},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dataset {
    Penguins,
    Titanic,
}

impl Dataset {
    pub const ALL: [Self; 2] = [Self::Penguins, Self::Titanic];
    pub fn name(self) -> &'static str {
        match self {
            Self::Penguins => "Penguins",
            Self::Titanic => "Titanic",
        }
    }
    pub fn csv(self) -> &'static str {
        match self {
            Self::Penguins => include_str!("../data/penguins.csv"),
            Self::Titanic => include_str!("../data/titanic.csv"),
        }
    }
    pub fn document(self) -> Document {
        parse_csv(self.csv()).expect("bundled dataset must satisfy document limits")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ImportError(String);
impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ImportError {}

/// Preserves the header, strings and empty fields. No per-cell commands or partial writes.
/// This is a document builder for the bundled samples, not a browser file-upload API.
pub fn parse_csv(source: &str) -> Result<Document, ImportError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(source.trim_start_matches('\u{feff}').as_bytes());
    let mut cells = BTreeMap::new();
    let (mut rows, mut columns) = (0, 0);
    for record in reader.records() {
        let record = record.map_err(|error| ImportError(format!("Некорректный CSV: {error}")))?;
        rows += 1;
        columns = record.len();
        if rows > MAX_DIMENSION || columns > MAX_DIMENSION {
            return Err(ImportError("CSV превышает размеры листа".into()));
        }
        for (column, value) in record.iter().enumerate() {
            if value.chars().count() > 512 {
                return Err(ImportError(
                    "Содержимое ячейки превышает 512 символов".into(),
                ));
            }
            if !value.is_empty() {
                cells.insert(
                    Address::new(rows, column + 1).to_string(),
                    value.to_string(),
                );
            }
            if cells.len() > MAX_FILLED_CELLS {
                return Err(ImportError("CSV превышает число заполненных ячеек".into()));
            }
        }
    }
    let document = Document {
        rows,
        columns,
        cells,
    };
    if !document.validate() {
        return Err(ImportError(
            "CSV пуст или превышает число заполненных ячеек".into(),
        ));
    }
    Ok(document)
}

/// Extends old snapshots without replacing existing data, renames or active selection.
pub fn append_missing(snapshot: &mut Snapshot) -> bool {
    let mut changed = false;
    for dataset in Dataset::ALL {
        if snapshot
            .sheets
            .iter()
            .any(|sheet| sheet.dataset == Some(dataset))
            || snapshot.sheets.len() >= 50
        {
            continue;
        }
        let Some(id) = snapshot
            .sheets
            .iter()
            .map(|sheet| sheet.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .filter(|id| *id < u64::MAX)
        else {
            continue;
        };
        snapshot.sheets.push(SavedSheet {
            id,
            name: dataset.name().into(),
            dataset: Some(dataset),
            document: dataset.document(),
            selected: Address::new(1, 1),
            top: 1,
            left: 1,
        });
        changed = true;
    }
    changed
}
