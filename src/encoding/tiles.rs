use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tiles {
    pub id: String,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub offset: Option<[u32; 2]>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub size: Option<[u32; 2]>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_index: Option<u32>,
    pub tile_size: [u32; 2],
    pub tiles: Vec<Vec<super::hybrid::Row>>,
    pub map: Vec<Vec<u32>>,
}
