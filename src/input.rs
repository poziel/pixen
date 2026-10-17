use crate::schema::{self, Document};
use anyhow::Result;
use std::path::Path;
pub fn is_container(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("spritex"))
}
pub fn read(path: &Path) -> Result<Document> {
    if is_container(path) {
        crate::container::Reader::open(path)?.load()
    } else {
        schema::read_document(&crate::output::read_bounded(path, schema::MAX_JSON)?)
    }
}
pub fn compile(path: &Path) -> Result<crate::model::Sprite> {
    schema::compile(read(path)?)
}
