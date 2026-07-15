use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value as JsonValue;

use super::workspace_dto::{ViewIcon, ViewLayout};

/// Mirrors `PublishedView` in AppFlowy-Web's
/// `src/application/publish-snapshot/types.ts`.
///
/// `child_views`/`ancestor_views` are intentionally left empty: AppFlowy-Web
/// resolves outline/breadcrumb data from the separate `published-outline`
/// endpoint and only falls back to these fields when that call fails.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedViewDto {
  pub view_id: String,
  pub name: String,
  pub icon: Option<ViewIcon>,
  pub extra: Option<String>,
  pub layout: ViewLayout,
  pub child_views: Vec<JsonValue>,
  pub ancestor_views: Vec<JsonValue>,
  pub visible_view_ids: Vec<String>,
  pub database_relations: HashMap<String, String>,
}

/// Mirrors `PublishedDocumentSnapshotPayload.document`.
///
/// `children` is the Slate node tree the editor renders directly.
/// `raw` mirrors `collab_document::blocks::DocumentData` and is used by
/// AppFlowy-Web when rendering documents embedded as database rows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedDocumentDto {
  pub children: Vec<JsonValue>,
  pub raw: JsonValue,
}

/// Mirrors `PublishedDatabaseSnapshotPayload.database.raw`.
///
/// Each value is a direct JSON dump of the underlying Yrs "data" root map
/// (via `Collab::to_json_value`), which is exactly the structure
/// AppFlowy-Web's `database-yjs-render-bridge.ts` rehydrates back into Yjs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedDatabaseRawDto {
  pub database: JsonValue,
  pub rows: HashMap<String, JsonValue>,
  pub row_documents: HashMap<String, JsonValue>,
}

/// Mirrors `PublishedDatabaseSnapshotPayload.database`.
///
/// `fields`/`views`/`rows` are best-effort normalized summaries; as of this
/// writing AppFlowy-Web's renderer only consumes `raw` to hydrate the live
/// document, so these are not required for correct rendering.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedDatabaseDto {
  pub database_id: String,
  pub active_view_id: String,
  pub visible_view_ids: Vec<String>,
  pub fields: Vec<JsonValue>,
  pub views: Vec<JsonValue>,
  pub rows: Vec<JsonValue>,
  pub raw: PublishedDatabaseRawDto,
}

/// Mirrors `PublishedPageSnapshotPayload` (the discriminated union of
/// `PublishedDocumentSnapshotPayload` and `PublishedDatabaseSnapshotPayload`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PublishedPageSnapshotDto {
  #[serde(rename = "document", rename_all = "camelCase")]
  Document {
    schema_version: u8,
    namespace: String,
    publish_name: String,
    view: PublishedViewDto,
    document: PublishedDocumentDto,
  },
  #[serde(rename = "database", rename_all = "camelCase")]
  Database {
    schema_version: u8,
    namespace: String,
    publish_name: String,
    view: PublishedViewDto,
    database: PublishedDatabaseDto,
  },
}
