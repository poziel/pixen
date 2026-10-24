//! Deserialize a payload straight into the existing encoding structs. The streaming
//! preflight supplies its encoding and rejects payload IDs before this adapter runs.
use crate::{
    encoding,
    schema::{CelDefinition, LegacyCel},
};
use anyhow::{Result, bail};
use serde::{
    Deserialize,
    de::{
        DeserializeSeed, IgnoredAny, MapAccess, Visitor,
        value::{MapAccessDeserializer, StrDeserializer, StringDeserializer},
    },
};
use std::{fmt, marker::PhantomData};

struct Payload<'a, T> {
    id: &'a str,
    marker: PhantomData<T>,
}
impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for Payload<'_, T> {
    type Value = T;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<T, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de, T: Deserialize<'de>> Visitor<'de> for Payload<'_, T> {
    type Value = T;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a cel payload object")
    }
    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
        T::deserialize(MapAccessDeserializer::new(Fields {
            inner: map,
            id: Some(self.id),
            pending_id: false,
        }))
    }
}
struct Fields<'a, A> {
    inner: A,
    id: Option<&'a str>,
    pending_id: bool,
}
impl<'de, A: MapAccess<'de>> MapAccess<'de> for Fields<'_, A> {
    type Error = A::Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        if self.id.is_some() {
            self.pending_id = true;
            return seed.deserialize(StrDeserializer::new("id")).map(Some);
        }
        while let Some(key) = self.inner.next_key::<String>()? {
            // The header was already checked and selected the exact target type.
            if key == "encoding" {
                self.inner.next_value::<IgnoredAny>()?;
                continue;
            }
            return seed.deserialize(StringDeserializer::new(key)).map(Some);
        }
        Ok(None)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        if self.pending_id {
            self.pending_id = false;
            return seed.deserialize(StrDeserializer::new(self.id.take().unwrap()));
        }
        self.inner.next_value_seed(seed)
    }
}
fn typed<'de, T: Deserialize<'de>>(bytes: &'de [u8], id: &str) -> Result<T> {
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let result = Payload::<T> {
        id,
        marker: PhantomData,
    }
    .deserialize(&mut d)?;
    d.end()?;
    Ok(result)
}
pub(super) fn read(bytes: &[u8], id: &str, version: u32) -> Result<CelDefinition> {
    let kind = crate::encoding::preflight::check_cel(bytes)?;
    if version == 1 {
        anyhow::ensure!(kind.is_none(), "version 1 requires legacy grid cels");
        return Ok(CelDefinition::Legacy(typed::<LegacyCel>(bytes, id)?));
    }
    use encoding::Encoding;
    let encoded = match kind.as_deref() {
        Some("grid") => Encoding::Grid(typed(bytes, id)?),
        Some("hybrid-rle") => Encoding::HybridRle(typed(bytes, id)?),
        Some("sparse-rows") => Encoding::SparseRows(typed(bytes, id)?),
        Some("row-table") => Encoding::RowTable(typed(bytes, id)?),
        Some("tiles") => Encoding::Tiles(typed(bytes, id)?),
        Some(other) => bail!("unsupported cel encoding {other:?}"),
        None => bail!("version 2 requires explicit encoding"),
    };
    Ok(CelDefinition::Encoded(encoded))
}
