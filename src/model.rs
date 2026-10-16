use crate::schema::{Annotations, Frame, Layer, PixeloramaExtras, Tag};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone)]
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub annotations: Annotations,
    pub pixelorama: PixeloramaExtras,
    pub frames: Vec<Frame>,
    pub layers: Vec<Layer>,
    pub tags: Vec<Tag>,
    pub images: BTreeMap<String, Arc<[u8]>>,
}
