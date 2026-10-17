//! Version-2 cel storage, decoded directly to root-palette indices.
pub mod grid;
pub mod hybrid;
pub mod hybrid_rle;
pub(crate) mod preflight;
pub mod row_table;
pub mod sparse_rows;
pub mod tiles;
mod writer;
pub use writer::encode;

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

use crate::schema::{Canvas, MAX_GRID_PIXELS, MAX_SLOTS, pixel_bytes};
use anyhow::{Context, Result, ensure};
use hybrid::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "encoding")]
pub enum Encoding {
    #[serde(rename = "grid")]
    Grid(grid::Grid),
    #[serde(rename = "hybrid-rle")]
    HybridRle(hybrid_rle::HybridRle),
    #[serde(rename = "sparse-rows")]
    SparseRows(sparse_rows::SparseRows),
    #[serde(rename = "row-table")]
    RowTable(row_table::RowTable),
    #[serde(rename = "tiles")]
    Tiles(tiles::Tiles),
}

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Grid,
    HybridRle,
    SparseRows,
    RowTable,
    Tiles { tile_size: [u32; 2] },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Placement {
    pub offset: Option<[u32; 2]>,
    pub size: Option<[u32; 2]>,
    pub default_index: Option<u32>,
}

#[derive(Clone, Copy)]
struct Region {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    cropped: bool,
}
impl Placement {
    fn region(self, canvas: &Canvas, palette: usize) -> Result<Region> {
        pixel_bytes(canvas.width, canvas.height)?;
        ensure!(
            self.offset.is_some() == self.size.is_some(),
            "offset and size must be supplied together"
        );
        let [x, y] = self.offset.unwrap_or([0, 0]);
        let [w, h] = self.size.unwrap_or([canvas.width, canvas.height]);
        ensure!(w > 0 && h > 0, "region size must be positive");
        ensure!(
            x.checked_add(w).is_some_and(|n| n <= canvas.width)
                && y.checked_add(h).is_some_and(|n| n <= canvas.height),
            "region outside canvas"
        );
        if let Some(i) = self.default_index {
            ensure!((i as usize) < palette, "invalid default_index");
        }
        Ok(Region {
            x: x as usize,
            y: y as usize,
            w: w as usize,
            h: h as usize,
            cropped: w != canvas.width || h != canvas.height,
        })
    }
    fn check_default(self, required: bool) -> Result<()> {
        ensure!(
            self.default_index.is_some() == required,
            "default_index is required exactly for sparse omissions, outside-region fill or tile padding"
        );
        Ok(())
    }
}

/// Cumulative per-document storage budgets, including unused dictionary entries.
#[derive(Default)]
pub struct Budget {
    definitions: usize,
    pixels: usize,
    tokens: usize,
    references: usize,
}
impl Budget {
    fn add(value: &mut usize, count: usize, max: usize, name: &str) -> Result<()> {
        *value = value
            .checked_add(count)
            .context("storage budget overflow")?;
        ensure!(*value <= max, "{name} budget exceeded");
        Ok(())
    }
    fn definitions(&mut self, count: usize) -> Result<()> {
        Self::add(
            &mut self.definitions,
            count,
            MAX_SLOTS,
            "dictionary definitions",
        )
    }
    fn pixels(&mut self, count: usize) -> Result<()> {
        Self::add(
            &mut self.pixels,
            count,
            MAX_GRID_PIXELS,
            "expanded dictionary pixels",
        )
    }
    fn tokens(&mut self, count: usize) -> Result<()> {
        Self::add(&mut self.tokens, count, MAX_GRID_PIXELS, "tokens")
    }
    fn references(&mut self, count: usize) -> Result<()> {
        Self::add(&mut self.references, count, MAX_GRID_PIXELS, "references")
    }
    fn row(&mut self, row: &Row, width: usize, palette: usize) -> Result<()> {
        self.tokens(row.len())?;
        ensure!(
            hybrid::width(row, width, palette)? == width,
            "underfilled row"
        );
        Ok(())
    }
}

impl Encoding {
    pub fn id(&self) -> &str {
        match self {
            Self::Grid(c) => &c.id,
            Self::HybridRle(c) => &c.id,
            Self::SparseRows(c) => &c.id,
            Self::RowTable(c) => &c.id,
            Self::Tiles(c) => &c.id,
        }
    }
    pub fn placement(&self) -> Placement {
        macro_rules! p {
            ($c:expr) => {
                Placement {
                    offset: $c.offset,
                    size: $c.size,
                    default_index: $c.default_index,
                }
            };
        }
        match self {
            Self::Grid(c) => p!(c),
            Self::HybridRle(c) => p!(c),
            Self::SparseRows(c) => p!(c),
            Self::RowTable(c) => p!(c),
            Self::Tiles(c) => p!(c),
        }
    }
    /// Validate all storage before allocating any expanded pixels.
    pub fn validate(&self, canvas: &Canvas, palette: usize, budget: &mut Budget) -> Result<()> {
        let p = self.placement();
        let r = p.region(canvas, palette)?;
        match self {
            Self::Grid(c) => {
                p.check_default(r.cropped)?;
                ensure!(c.pixels.len() == r.h, "wrong grid height");
                for row in &c.pixels {
                    budget.tokens(row.len())?;
                    ensure!(row.len() == r.w, "wrong grid width");
                    ensure!(
                        row.iter().all(|&i| (i as usize) < palette),
                        "invalid palette index"
                    );
                }
            }
            Self::HybridRle(c) => {
                p.check_default(r.cropped)?;
                ensure!(c.rows.len() == r.h, "wrong row height");
                for row in &c.rows {
                    budget.row(row, r.w, palette)?;
                }
            }
            Self::SparseRows(c) => {
                p.check_default(true)?;
                let mut prev_y = None;
                for row in &c.rows {
                    ensure!(
                        (row.y as usize) < r.h && prev_y.is_none_or(|y| row.y > y),
                        "unsorted or out-of-range sparse row"
                    );
                    prev_y = Some(row.y);
                    ensure!(!row.spans.is_empty(), "empty sparse row");
                    let mut prev_x = None;
                    let mut end = 0;
                    for span in &row.spans {
                        let x = span.x as usize;
                        ensure!(
                            x < r.w && x >= end && prev_x.is_none_or(|v| x > v),
                            "unsorted, overlapping or out-of-range span"
                        );
                        budget.tokens(span.pixels.len())?;
                        let n = hybrid::width(&span.pixels, r.w - x, palette)?;
                        ensure!(n > 0, "empty span");
                        end = x + n;
                        prev_x = Some(x);
                    }
                }
            }
            Self::RowTable(c) => {
                p.check_default(r.cropped)?;
                budget.definitions(c.row_definitions.len())?;
                budget.pixels(
                    c.row_definitions
                        .len()
                        .checked_mul(r.w)
                        .context("dictionary size overflow")?,
                )?;
                budget.references(c.rows.len())?;
                ensure!(c.rows.len() == r.h, "wrong row reference count");
                ensure!(
                    c.rows
                        .iter()
                        .all(|&i| (i as usize) < c.row_definitions.len()),
                    "invalid row reference"
                );
                for row in &c.row_definitions {
                    budget.row(row, r.w, palette)?;
                }
            }
            Self::Tiles(c) => {
                let [tw, th] = c.tile_size;
                pixel_bytes(tw, th)?;
                let (tw, th) = (tw as usize, th as usize);
                let (mw, mh) = (r.w.div_ceil(tw), r.h.div_ceil(th));
                p.check_default(r.cropped || r.w % tw != 0 || r.h % th != 0)?;
                budget.definitions(c.tiles.len())?;
                budget.pixels(
                    c.tiles
                        .len()
                        .checked_mul(tw * th)
                        .context("tile dictionary overflow")?,
                )?;
                budget.references(mw * mh)?;
                ensure!(c.map.len() == mh, "wrong map height");
                for row in &c.map {
                    ensure!(row.len() == mw, "wrong map width");
                    ensure!(
                        row.iter().all(|&i| (i as usize) < c.tiles.len()),
                        "invalid tile reference"
                    );
                }
                for tile in &c.tiles {
                    ensure!(tile.len() == th, "wrong tile height");
                    for row in tile {
                        budget.row(row, tw, palette)?;
                    }
                }
                // Inspect padding directly in the tokens; never expand an unvalidated run.
                for (my, row) in c.map.iter().enumerate() {
                    for (mx, &index) in row.iter().enumerate() {
                        if (mx + 1) * tw <= r.w && (my + 1) * th <= r.h {
                            continue;
                        }
                        for (ty, tokens) in c.tiles[index as usize].iter().enumerate() {
                            let mut tx = 0;
                            for token in tokens {
                                let (i, n) = match *token {
                                    hybrid::Token::Literal(i) => (i, 1),
                                    hybrid::Token::Run([i, n]) => (i, n as usize),
                                };
                                if my * th + ty >= r.h || mx * tw + tx + n > r.w {
                                    ensure!(
                                        Some(i) == p.default_index,
                                        "non-default boundary padding"
                                    );
                                }
                                tx += n;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn decode(&self, canvas: &Canvas, palette: usize) -> Result<Vec<Vec<u32>>> {
        self.validate(canvas, palette, &mut Budget::default())?;
        self.decode_validated(canvas, palette)
    }
    pub(crate) fn decode_validated(
        &self,
        canvas: &Canvas,
        palette: usize,
    ) -> Result<Vec<Vec<u32>>> {
        let p = self.placement();
        let r = p.region(canvas, palette)?;
        let mut image =
            vec![vec![p.default_index.unwrap_or(0); canvas.width as usize]; canvas.height as usize];
        let mut put = |y: usize, x: usize, row: &[u32]| {
            image[r.y + y][r.x + x..r.x + x + row.len()].copy_from_slice(row);
        };
        match self {
            Self::Grid(c) => {
                for (y, row) in c.pixels.iter().enumerate() {
                    put(y, 0, row);
                }
            }
            Self::HybridRle(c) => {
                for (y, row) in c.rows.iter().enumerate() {
                    put(y, 0, &hybrid::expand(row));
                }
            }
            Self::SparseRows(c) => {
                for row in &c.rows {
                    for span in &row.spans {
                        put(
                            row.y as usize,
                            span.x as usize,
                            &hybrid::expand(&span.pixels),
                        );
                    }
                }
            }
            Self::RowTable(c) => {
                for (y, &i) in c.rows.iter().enumerate() {
                    put(y, 0, &hybrid::expand(&c.row_definitions[i as usize]));
                }
            }
            Self::Tiles(c) => {
                let [tw, th] = c.tile_size;
                let (tw, th) = (tw as usize, th as usize);
                for (my, row) in c.map.iter().enumerate() {
                    for (mx, &i) in row.iter().enumerate() {
                        for (ty, tokens) in c.tiles[i as usize].iter().enumerate() {
                            if my * th + ty < r.h {
                                let row = hybrid::expand(tokens);
                                put(my * th + ty, mx * tw, &row[..tw.min(r.w - mx * tw)]);
                            }
                        }
                    }
                }
            }
        }
        Ok(image)
    }
}
