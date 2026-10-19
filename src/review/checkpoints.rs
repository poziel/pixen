use super::*;
use std::collections::BTreeMap;
pub const MAX_CHECKPOINTS: usize = 16;
pub const MAX_CHECKPOINT_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum CheckpointAction {
    Create {
        expected_revision: u64,
        label: String,
    },
    List,
    Delete {
        expected_revision: u64,
        checkpoint_id: String,
    },
}
struct Checkpoint {
    label: String,
    revision: u64,
    bytes: Vec<u8>,
}
#[derive(Default)]
pub struct CheckpointStore {
    next: u64,
    items: BTreeMap<u64, Checkpoint>,
    bytes: usize,
}
impl CheckpointStore {
    pub fn create(&mut self, doc: &Document, revision: u64, label: &str) -> Result<Value> {
        let label = label.trim();
        ensure!(
            !label.is_empty()
                && label.chars().count() <= 80
                && !label.chars().any(char::is_control),
            "checkpoint label must be 1..80 characters on one line"
        );
        ensure!(
            self.items.len() < MAX_CHECKPOINTS,
            "checkpoint limit is 16; explicitly delete a baseline first"
        );
        let bytes = serde_json::to_vec(doc)?;
        ensure!(
            self.bytes + bytes.len() <= MAX_CHECKPOINT_BYTES,
            "checkpoint snapshots exceed 64 MiB; explicitly delete a baseline first"
        );
        let n = self.next + 1;
        let size = bytes.len();
        self.items.insert(
            n,
            Checkpoint {
                label: label.into(),
                revision,
                bytes,
            },
        );
        self.next = n;
        self.bytes += size;
        Ok(
            json!({"checkpoint_id":format!("checkpoint-{n}"),"label":label,"creation_revision":revision,"bytes":size}),
        )
    }
    pub fn list(&self) -> Value {
        json!({"checkpoints":self.items.iter().map(|(n,c)|json!({"checkpoint_id":format!("checkpoint-{n}"),"label":c.label,"creation_revision":c.revision,"bytes":c.bytes.len()})).collect::<Vec<_>>(),"max_checkpoints":MAX_CHECKPOINTS,"max_bytes":MAX_CHECKPOINT_BYTES,"available_bytes":MAX_CHECKPOINT_BYTES-self.bytes,"session_local":true})
    }
    fn key(id: &str) -> Result<u64> {
        id.strip_prefix("checkpoint-")
            .and_then(|s| s.parse().ok())
            .context("unknown checkpoint_id")
    }
    pub fn delete(&mut self, id: &str) -> Result<Value> {
        let c = self
            .items
            .remove(&Self::key(id)?)
            .context("unknown checkpoint_id")?;
        self.bytes -= c.bytes.len();
        Ok(json!({"deleted":id}))
    }
    pub fn document(&self, id: &str) -> Result<(Document, u64)> {
        let c = self
            .items
            .get(&Self::key(id)?)
            .context("unknown checkpoint_id")?;
        Ok((crate::schema::read_document(&c.bytes)?, c.revision))
    }
}
