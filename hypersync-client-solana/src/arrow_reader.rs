use std::collections::BTreeMap;
use std::io::Cursor;

use anyhow::{Context, Result};
use arrow::ipc::reader::FileReader;
use arrow::record_batch::RecordBatch;

use crate::types::{ArrowResponseData, QueryResponse};

/// Decode a binary Arrow IPC bundle from the server into a `QueryResponse`.
///
/// Wire format:
/// ```text
/// [next_slot: u64 LE][num_tables: u32 LE]
/// For each table:
///   [name_len: u32 LE][name: UTF-8][ipc_len: u64 LE][ipc_data: Arrow IPC file bytes]
/// ```
pub fn decode_response(data: &[u8]) -> Result<QueryResponse> {
    let mut pos = 0;
    let response_bytes = data.len();

    anyhow::ensure!(data.len() >= 12, "data too short for header");

    let next_slot = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let num_tables = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let mut tables = BTreeMap::new();

    for _ in 0..num_tables {
        anyhow::ensure!(pos + 4 <= data.len(), "truncated table name length");
        let name_len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;

        anyhow::ensure!(pos + name_len <= data.len(), "truncated table name");
        let name = std::str::from_utf8(&data[pos..pos + name_len]).context("invalid table name")?;
        pos += name_len;

        anyhow::ensure!(pos + 8 <= data.len(), "truncated IPC length");
        let ipc_len = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
        pos += 8;

        anyhow::ensure!(pos + ipc_len <= data.len(), "truncated IPC data");
        let ipc_data = &data[pos..pos + ipc_len];
        pos += ipc_len;

        let cursor = Cursor::new(ipc_data);
        let reader = FileReader::try_new(cursor, None)
            .with_context(|| format!("open IPC reader for {}", name))?;

        let mut combined: Option<RecordBatch> = None;
        for batch_result in reader {
            let batch = batch_result.with_context(|| format!("read IPC batch for {}", name))?;
            if batch.num_rows() == 0 {
                continue;
            }
            combined = Some(match combined {
                None => batch,
                Some(prev) => arrow::compute::concat_batches(&prev.schema(), &[prev, batch])
                    .with_context(|| format!("concat batches for {}", name))?,
            });
        }

        let static_name: &'static str = hypersync_solana_schema::TABLE_NAMES
            .iter()
            .find(|&&n| n == name)
            .copied()
            .unwrap_or_else(|| Box::leak(name.to_owned().into_boxed_str()));

        if let Some(batch) = combined {
            tables.insert(static_name, batch);
        } else {
            let schema = hypersync_solana_schema::schema_for_table(name).unwrap_or_else(|| {
                arrow::datatypes::SchemaRef::new(arrow::datatypes::Schema::empty())
            });
            tables.insert(static_name, RecordBatch::new_empty(schema));
        }
    }

    Ok(QueryResponse {
        next_slot,
        data: ArrowResponseData { tables },
        response_bytes,
    })
}
