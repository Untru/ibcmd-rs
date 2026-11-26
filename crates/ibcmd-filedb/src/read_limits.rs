//! Optional cumulative allocation/work budget for untrusted read-only inputs.
use crate::{Error, Result};

/// Cumulative limits for one reader, including table descriptors and repeated
/// CONFIG/CONFIGSAVE reads. max_records counts descriptors and live row records.
/// Object bytes cover descriptors, records and blob-object buffers; binary bytes
/// cover copied row values, including multiple references to the same blob.
#[derive(Clone, Copy, Debug)]
pub struct ReadLimits {
    pub max_records: usize,
    pub max_binary_bytes: u64,
    pub max_object_bytes: u64,
}

#[derive(Clone)]
pub(crate) struct ReadBudget {
    state: std::sync::Arc<std::sync::Mutex<Counters>>,
    check: std::sync::Arc<dyn Fn() -> Result<()> + Send + Sync>,
}
struct Counters {
    records: usize,
    binary_bytes: u64,
    object_bytes: u64,
}
impl ReadBudget {
    pub(crate) fn new(
        limits: ReadLimits,
        check: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(Counters {
                records: limits.max_records,
                binary_bytes: limits.max_binary_bytes,
                object_bytes: limits.max_object_bytes,
            })),
            check: std::sync::Arc::new(check),
        }
    }
    pub(crate) fn check(&self) -> Result<()> {
        (self.check)()
    }
    pub(crate) fn object(&mut self, bytes: u64) -> Result<()> {
        self.check()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error::Refused("read budget lock poisoned".into()))?;
        state.object_bytes = state
            .object_bytes
            .checked_sub(bytes)
            .ok_or_else(|| Error::Refused("file database object materialization limit".into()))?;
        Ok(())
    }
    pub(crate) fn record(&mut self, bytes: u64) -> Result<()> {
        self.check()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| Error::Refused("read budget lock poisoned".into()))?;
        let records = state
            .records
            .checked_sub(1)
            .ok_or_else(|| Error::Refused("file database record limit".into()))?;
        let binary = state
            .binary_bytes
            .checked_sub(bytes)
            .ok_or_else(|| Error::Refused("file database binary materialization limit".into()))?;
        state.records = records;
        state.binary_bytes = binary;
        Ok(())
    }
}
