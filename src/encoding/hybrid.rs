//! Shared horizontal literal/run tokens. Counts never drive allocation unchecked.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Token {
    Literal(u32),
    Run([u32; 2]),
}
pub type Row = Vec<Token>;

pub fn width(row: &[Token], limit: usize, palette: usize) -> Result<usize> {
    let mut width = 0usize;
    for token in row {
        let (index, count) = match *token {
            Token::Literal(i) => (i, 1),
            Token::Run([i, n]) => (i, n as usize),
        };
        ensure!((index as usize) < palette, "invalid palette index {index}");
        ensure!(count > 0, "zero run");
        width = width
            .checked_add(count)
            .ok_or_else(|| anyhow::anyhow!("row overflow"))?;
        ensure!(width <= limit, "row exceeds required width");
    }
    Ok(width)
}

pub(crate) fn expand(row: &[Token]) -> Vec<u32> {
    row.iter()
        .flat_map(|t| match *t {
            Token::Literal(i) => std::iter::repeat_n(i, 1),
            Token::Run([i, n]) => std::iter::repeat_n(i, n as usize),
        })
        .collect()
}

pub fn encode(row: &[u32]) -> Row {
    let mut result = Vec::new();
    let mut x = 0;
    while x < row.len() {
        let index = row[x];
        let count = row[x..].iter().take_while(|&&i| i == index).count();
        let digits = index.to_string().len();
        // Both alternatives have the same surrounding separators. Internal commas count.
        let literals = count * digits + count - 1;
        let run = digits + count.to_string().len() + 3;
        if run < literals {
            result.push(Token::Run([index, count as u32]));
        } else {
            result.extend(std::iter::repeat_n(Token::Literal(index), count));
        }
        x += count;
    }
    result
}
