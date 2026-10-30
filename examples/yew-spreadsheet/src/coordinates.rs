use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

pub const MAX_DIMENSION: usize = 1000;
pub const WINDOW_ROWS: usize = 8;
pub const WINDOW_COLUMNS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Address {
    pub row: usize,
    pub column: usize,
}

impl Address {
    pub const fn new(row: usize, column: usize) -> Self {
        Self { row, column }
    }
}
impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", column_name(self.column), self.row)
    }
}
impl FromStr for Address {
    type Err = ();
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let source = source.trim().to_ascii_uppercase();
        let letters = source.bytes().take_while(u8::is_ascii_alphabetic).count();
        if letters == 0 || letters == source.len() {
            return Err(());
        }
        let mut column = 0_usize;
        for c in source[..letters].bytes() {
            column = column
                .checked_mul(26)
                .and_then(|n| n.checked_add((c - b'A' + 1) as usize))
                .ok_or(())?;
        }
        let suffix = &source[letters..];
        if !suffix.bytes().all(|c| c.is_ascii_digit()) {
            return Err(());
        }
        let row = suffix.parse().map_err(|_| ())?;
        if row == 0 {
            return Err(());
        }
        Ok(Self { row, column })
    }
}

pub fn column_name(mut column: usize) -> String {
    let mut name = Vec::new();
    while column > 0 {
        column -= 1;
        name.push(b'A' + (column % 26) as u8);
        column /= 26;
    }
    name.reverse();
    String::from_utf8(name).unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    pub rows: usize,
    pub columns: usize,
}
impl Default for Dimensions {
    fn default() -> Self {
        Self {
            rows: 100,
            columns: 40,
        }
    }
}
impl Dimensions {
    pub fn contains(self, address: Address) -> bool {
        (1..=self.rows).contains(&address.row) && (1..=self.columns).contains(&address.column)
    }
    pub fn clamp(self, address: Address) -> Address {
        Address::new(
            address.row.clamp(1, self.rows),
            address.column.clamp(1, self.columns),
        )
    }
    pub fn valid(self) -> bool {
        (1..=MAX_DIMENSION).contains(&self.rows) && (1..=MAX_DIMENSION).contains(&self.columns)
    }
}
