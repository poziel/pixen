//! Streaming resource checks before typed deserialization buffers pixel arrays.
use crate::schema::{MAX_FRAMES, MAX_GRID_PIXELS, MAX_LAYERS, MAX_SLOTS};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::fmt;

pub fn check(bytes: &[u8]) -> anyhow::Result<()> {
    scan(bytes, &mut Counts::default())
}
fn scan(bytes: &[u8], counts: &mut Counts) -> anyhow::Result<()> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    Scan {
        counts,
        limit: MAX_GRID_PIXELS,
        context: Context::Root,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(())
}
struct Counts {
    values: usize,
    definitions: usize,
    tokens: usize,
    references: usize,
    pixel_limit: usize,
}
impl Default for Counts {
    fn default() -> Self {
        Self {
            values: 0,
            definitions: 0,
            tokens: 0,
            references: 0,
            pixel_limit: MAX_GRID_PIXELS,
        }
    }
}
#[derive(Clone, Copy)]
enum Context {
    Root,
    Cels,
    Cel,
    RowDictionary,
    TileDictionary,
    Tile,
    PixelRows,
    PixelRow,
    Rows,
    RowEntry,
    Map,
    References,
    Spans,
    Span,
    Other,
}
struct Scan<'a> {
    counts: &'a mut Counts,
    limit: usize,
    context: Context,
}
impl<'de> DeserializeSeed<'de> for Scan<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        self.counts.values += 1;
        if self.counts.values > MAX_GRID_PIXELS * 4 {
            return Err(serde::de::Error::custom("JSON structural budget exceeded"));
        }
        d.deserialize_any(self)
    }
}
impl Scan<'_> {
    fn scalar<E: serde::de::Error>(self) -> Result<(), E> {
        if matches!(self.context, Context::RowEntry) {
            self.counts.references += 1;
            if self.counts.references > self.counts.pixel_limit {
                return Err(E::custom("references budget exceeded"));
            }
        }
        Ok(())
    }
}
impl<'de> Visitor<'de> for Scan<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<(), E> {
        self.scalar()
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<(), E> {
        self.scalar()
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<(), E> {
        self.scalar()
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<(), E> {
        self.scalar()
    }
    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<(), E> {
        self.scalar()
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        self.scalar()
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        let child = match self.context {
            Context::Cels => Context::Cel,
            Context::RowDictionary | Context::Tile | Context::PixelRows => Context::PixelRow,
            Context::TileDictionary => Context::Tile,
            Context::Rows => Context::RowEntry,
            Context::Map => Context::References,
            Context::Spans => Context::Span,
            _ => Context::Other,
        };
        let mut n = 0;
        loop {
            if n == self.limit {
                if seq.next_element::<IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::custom("JSON array budget exceeded"));
                }
                break;
            }
            if seq
                .next_element_seed(Scan {
                    counts: self.counts,
                    limit: MAX_GRID_PIXELS,
                    context: child,
                })?
                .is_none()
            {
                break;
            }
            if matches!(
                self.context,
                Context::RowDictionary | Context::TileDictionary
            ) {
                self.counts.definitions += 1;
                if self.counts.definitions > MAX_SLOTS {
                    return Err(serde::de::Error::custom(
                        "dictionary definitions budget exceeded",
                    ));
                }
            }
            if matches!(self.context, Context::PixelRow | Context::RowEntry) {
                self.counts.tokens += 1;
                if self.counts.tokens > self.counts.pixel_limit {
                    return Err(serde::de::Error::custom("tokens budget exceeded"));
                }
            }
            if matches!(self.context, Context::References) {
                self.counts.references += 1;
                if self.counts.references > self.counts.pixel_limit {
                    return Err(serde::de::Error::custom("references budget exceeded"));
                }
            }
            n += 1;
        }
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
            let (context, limit) = match (self.context, key.as_ref()) {
                (Context::Root, "cels") => (Context::Cels, MAX_SLOTS),
                (Context::Root, "frames") => (Context::Other, MAX_FRAMES),
                (Context::Root, "layers") => (Context::Other, MAX_LAYERS),
                (Context::Root, "palette") => (Context::Other, 1_048_576),
                (Context::Cel, "row_definitions") => (Context::RowDictionary, MAX_SLOTS),
                (Context::Cel, "tiles") => (Context::TileDictionary, MAX_SLOTS),
                (Context::Cel, "pixels") => (Context::PixelRows, MAX_GRID_PIXELS),
                (Context::Cel, "rows") => (Context::Rows, MAX_GRID_PIXELS),
                (Context::Cel, "map") => (Context::Map, MAX_GRID_PIXELS),
                (Context::RowEntry, "spans") => (Context::Spans, MAX_GRID_PIXELS),
                (Context::Span, "pixels") => (Context::PixelRow, MAX_GRID_PIXELS),
                _ => (Context::Other, MAX_GRID_PIXELS),
            };
            map.next_value_seed(Scan {
                counts: self.counts,
                limit,
                context,
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_limits_count_tokens_not_run_counts_and_include_unused_storage() {
        let json = br#"{"cels":[{"rows":[[0,[1,4294967295],2]]}]}"#;
        assert!(
            scan(
                json,
                &mut Counts {
                    pixel_limit: 3,
                    ..Default::default()
                }
            )
            .is_ok()
        );
        assert!(
            scan(
                json,
                &mut Counts {
                    pixel_limit: 2,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let json = br#"{"cels":[{"row_definitions":[[0,1],[2,2]],"rows":[0,0,0]}]}"#;
        assert!(
            scan(
                json,
                &mut Counts {
                    pixel_limit: 3,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let json = br#"{"cels":[{"tiles":[[[0]]],"map":[[0,0],[0,0]]}]}"#;
        assert!(
            scan(
                json,
                &mut Counts {
                    pixel_limit: 3,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let json=br#"{"annotations":{"metadata":{"tiles":[0,0,0,0]}},"cels":[{"rows":[{"spans":[{"pixels":[0,[1,2]]}]}]}]}"#;
        assert!(
            scan(
                json,
                &mut Counts {
                    pixel_limit: 2,
                    ..Default::default()
                }
            )
            .is_ok()
        );
    }
}
