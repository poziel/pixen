use super::*;
use std::collections::HashMap;

/// Encode exact full-canvas indices with a caller-selected encoding and placement.
/// This is a deterministic writer, not an encoding/placement optimizer.
pub fn encode(
    id: String,
    image: &[Vec<u32>],
    canvas: &Canvas,
    palette: usize,
    kind: Kind,
    placement: Placement,
) -> Result<Encoding> {
    crate::schema::identifier(&id)?;
    ensure!((1..=1_048_576).contains(&palette), "invalid palette size");
    let r = placement.region(canvas, palette)?;
    if !matches!(kind, Kind::Tiles { .. }) {
        placement.check_default(r.cropped || matches!(kind, Kind::SparseRows))?;
    }
    ensure!(image.len() == canvas.height as usize, "wrong image height");
    for (y, row) in image.iter().enumerate() {
        ensure!(row.len() == canvas.width as usize, "wrong image width");
        for (x, &i) in row.iter().enumerate() {
            ensure!((i as usize) < palette, "invalid palette index");
            if x < r.x || x >= r.x + r.w || y < r.y || y >= r.y + r.h {
                ensure!(
                    Some(i) == placement.default_index,
                    "cropping would discard non-default pixels"
                );
            }
        }
    }
    let Placement {
        offset,
        size,
        default_index,
    } = placement;
    macro_rules! cel { ($module:ident::$ty:ident, $($field:ident:$value:expr),* $(,)?) => {
        $module::$ty { id,offset,size,default_index,$($field:$value),* }
    }; }
    let region = || image[r.y..r.y + r.h].iter().map(|row| &row[r.x..r.x + r.w]);
    let encoding = match kind {
        Kind::Grid => {
            Encoding::Grid(cel!(grid::Grid,pixels:region().map(<[u32]>::to_vec).collect()))
        }
        Kind::HybridRle => Encoding::HybridRle(
            cel!(hybrid_rle::HybridRle,rows:region().map(hybrid::encode).collect()),
        ),
        Kind::SparseRows => {
            let default = default_index.context("sparse-rows requires default_index")?;
            let mut rows = Vec::new();
            for (y, row) in region().enumerate() {
                let mut spans = Vec::new();
                let mut x = 0;
                while x < r.w {
                    if row[x] == default {
                        x += 1;
                        continue;
                    }
                    let start = x;
                    while x < r.w && row[x] != default {
                        x += 1;
                    }
                    spans.push(sparse_rows::Span {
                        x: start as u32,
                        pixels: hybrid::encode(&row[start..x]),
                    });
                }
                if !spans.is_empty() {
                    rows.push(sparse_rows::SparseRow { y: y as u32, spans });
                }
            }
            Encoding::SparseRows(cel!(sparse_rows::SparseRows,rows:rows))
        }
        Kind::RowTable => {
            let mut definitions = Vec::new();
            let mut rows = Vec::new();
            let mut seen = HashMap::new();
            for row in region() {
                let next = definitions.len() as u32;
                let index = *seen.entry(row.to_vec()).or_insert_with(|| {
                    definitions.push(hybrid::encode(row));
                    next
                });
                rows.push(index);
            }
            Encoding::RowTable(cel!(row_table::RowTable,row_definitions:definitions,rows:rows))
        }
        Kind::Tiles { tile_size } => {
            let [tw, th] = tile_size;
            pixel_bytes(tw, th)?;
            let (tw, th) = (tw as usize, th as usize);
            let (mw, mh) = (r.w.div_ceil(tw), r.h.div_ceil(th));
            placement.check_default(r.cropped || r.w % tw != 0 || r.h % th != 0)?;
            // Charge the maximum candidate work before constructing tiles.
            ensure!(
                mw.checked_mul(mh)
                    .and_then(|n| n.checked_mul(tw * th))
                    .is_some_and(|n| n <= MAX_GRID_PIXELS),
                "tile writer pixel budget exceeded"
            );
            let mut tiles = Vec::new();
            let mut map = Vec::new();
            let mut seen = HashMap::new();
            for my in 0..mh {
                let mut refs = Vec::new();
                for mx in 0..mw {
                    let mut tile = vec![vec![default_index.unwrap_or(0); tw]; th];
                    for (ty, row) in tile.iter_mut().enumerate() {
                        for (tx, pixel) in row.iter_mut().enumerate() {
                            let (x, y) = (mx * tw + tx, my * th + ty);
                            if x < r.w && y < r.h {
                                *pixel = image[r.y + y][r.x + x];
                            }
                        }
                    }
                    let next = tiles.len() as u32;
                    let index = if let Some(&index) = seen.get(&tile) {
                        index
                    } else {
                        ensure!(
                            tiles.len() < MAX_SLOTS,
                            "dictionary definitions budget exceeded"
                        );
                        tiles.push(tile.iter().map(|row| hybrid::encode(row)).collect());
                        seen.insert(tile, next);
                        next
                    };
                    refs.push(index);
                }
                map.push(refs);
            }
            Encoding::Tiles(cel!(tiles::Tiles,tile_size:tile_size,tiles:tiles,map:map))
        }
    };
    crate::schema::identifier(encoding.id())?;
    encoding.validate(canvas, palette, &mut Budget::default())?;
    Ok(encoding)
}
