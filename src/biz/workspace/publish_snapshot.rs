use std::collections::HashMap;

use app_error::AppError;
use collab::core::collab::default_client_id;
use collab_document::blocks::{Block, DocumentData};
use collab_document::document::Document;
use database::publish::select_published_collab_workspace_view_id;
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use shared_entity::dto::publish_dto::PublishDatabaseData;
use shared_entity::dto::publish_snapshot_dto::{
  PublishedDatabaseDto, PublishedDatabaseRawDto, PublishedDocumentDto, PublishedPageSnapshotDto,
  PublishedViewDto,
};
use shared_entity::dto::workspace_dto::ViewLayout;
use uuid::Uuid;

use crate::biz::collab::utils::collab_from_doc_state;
use crate::biz::workspace::publish_dup::deserialize_publish_database_data;
use crate::state::AppState;

const SCHEMA_VERSION: u8 = 1;

/// Builds the JSON snapshot AppFlowy-Web's `v2/published/.../snapshot`
/// endpoint returns for a published page. This endpoint is what makes a
/// publish-page's public link actually render: the published record itself
/// (namespace + view) is already public and permission-agnostic, this only
/// packages its content into the schema AppFlowy-Web expects.
pub async fn get_published_page_snapshot(
  state: &AppState,
  publish_namespace: &str,
  publish_name: &str,
) -> Result<PublishedPageSnapshotDto, AppError> {
  let key =
    select_published_collab_workspace_view_id(&state.pg_pool, publish_namespace, publish_name)
      .await?;

  let (metadata, blob) = state
    .published_collab_store
    .get_collab_with_view_metadata_by_view_id(&key.view_id)
    .await?
    .ok_or_else(|| {
      AppError::RecordNotFound(format!(
        "Published collab not found for namespace: {}, publish_name: {}",
        publish_namespace, publish_name
      ))
    })?;

  let layout = metadata.view.layout.clone();
  match layout {
    ViewLayout::Document => {
      let document_data = decode_document_data(&blob, &key.view_id)?;
      let children = document_data_to_slate_children(&document_data);
      let raw = json!({ "data": document_data });

      Ok(PublishedPageSnapshotDto::Document {
        schema_version: SCHEMA_VERSION,
        namespace: publish_namespace.to_string(),
        publish_name: publish_name.to_string(),
        view: PublishedViewDto {
          view_id: metadata.view.view_id.clone(),
          name: metadata.view.name.clone(),
          icon: metadata.view.icon.clone(),
          extra: metadata.view.extra.clone(),
          layout: layout.clone(),
          child_views: vec![],
          ancestor_views: vec![],
          visible_view_ids: vec![],
          database_relations: HashMap::new(),
        },
        document: PublishedDocumentDto { children, raw },
      })
    },
    ViewLayout::Grid | ViewLayout::Board | ViewLayout::Calendar => {
      let db_payload = deserialize_publish_database_data(&blob)?;
      let database = build_database_dto(&key.view_id, &db_payload)?;
      let visible_view_ids = database.visible_view_ids.to_vec();
      let database_relations = db_payload
        .database_relations
        .iter()
        .map(|(db_id, view_id)| (db_id.to_string(), view_id.to_string()))
        .collect();

      Ok(PublishedPageSnapshotDto::Database {
        schema_version: SCHEMA_VERSION,
        namespace: publish_namespace.to_string(),
        publish_name: publish_name.to_string(),
        view: PublishedViewDto {
          view_id: metadata.view.view_id.clone(),
          name: metadata.view.name.clone(),
          icon: metadata.view.icon.clone(),
          extra: metadata.view.extra.clone(),
          layout: layout.clone(),
          child_views: vec![],
          ancestor_views: vec![],
          visible_view_ids,
          database_relations,
        },
        database,
      })
    },
    ViewLayout::Chat => Err(AppError::InvalidRequest(
      "AI Chat cannot be published".to_string(),
    )),
  }
}

fn decode_document_data(blob: &[u8], view_id: &Uuid) -> Result<DocumentData, AppError> {
  let collab = collab_from_doc_state(blob.to_vec(), view_id, default_client_id())?;
  let document = Document::open(collab).map_err(|e| AppError::Unhandled(e.to_string()))?;
  document
    .get_document_data()
    .map_err(|e| AppError::Unhandled(e.to_string()))
}

/// Decodes an arbitrary published collab blob into a full JSON dump of its
/// Yrs "data" root map via `Collab::to_json_value`. This is a generic
/// structural dump (Y.Map -> object, Y.Array -> array, primitives verbatim),
/// which is exactly the shape AppFlowy-Web's `database-yjs-render-bridge.ts`
/// re-hydrates back into a live Yjs document, so no per-schema mapping is
/// required on this side.
fn decode_collab_json(bytes: &[u8], object_id: &Uuid) -> Result<JsonValue, AppError> {
  let collab = collab_from_doc_state(bytes.to_vec(), object_id, default_client_id())?;
  Ok(collab.to_json_value())
}

fn build_database_dto(
  view_id: &Uuid,
  db_payload: &PublishDatabaseData,
) -> Result<PublishedDatabaseDto, AppError> {
  let db_json = decode_collab_json(&db_payload.database_collab, view_id)?;
  let database_value = db_json.get("database").cloned().unwrap_or(json!({}));
  let database_id = database_value
    .get("id")
    .and_then(|v| v.as_str())
    .unwrap_or_default()
    .to_string();

  let mut rows_raw: HashMap<String, JsonValue> = HashMap::new();
  for (row_id, bytes) in &db_payload.database_row_collabs {
    let row_json = decode_collab_json(bytes, row_id)?;
    rows_raw.insert(row_id.to_string(), row_json);
  }

  let mut row_documents: HashMap<String, JsonValue> = HashMap::new();
  for (row_id, bytes) in &db_payload.database_row_document_collabs {
    let document_data = decode_document_data(bytes, row_id)?;
    row_documents.insert(row_id.to_string(), json!({ "data": document_data }));
  }

  let fields = database_value
    .get("fields")
    .and_then(|v| v.as_object())
    .map(fields_to_summary)
    .unwrap_or_default();
  let views = database_value
    .get("views")
    .and_then(|v| v.as_object())
    .map(views_to_summary)
    .unwrap_or_default();
  let rows = rows_raw
    .iter()
    .map(|(row_id, row_json)| row_summary(row_id, row_json))
    .collect();

  let visible_view_ids = db_payload
    .visible_database_view_ids
    .iter()
    .map(|id| id.to_string())
    .collect();

  Ok(PublishedDatabaseDto {
    database_id,
    active_view_id: view_id.to_string(),
    visible_view_ids,
    fields,
    views,
    rows,
    raw: PublishedDatabaseRawDto {
      database: database_value,
      rows: rows_raw,
      row_documents,
    },
  })
}

fn fields_to_summary(fields: &JsonMap<String, JsonValue>) -> Vec<JsonValue> {
  fields
    .iter()
    .map(|(field_id, field)| {
      json!({
        "fieldId": field_id,
        "name": field.get("name").cloned().unwrap_or(json!("")),
        "fieldType": field.get("ty").cloned().unwrap_or(json!(0)),
        "isPrimary": field.get("is_primary").cloned().unwrap_or(json!(false)),
      })
    })
    .collect()
}

fn views_to_summary(views: &JsonMap<String, JsonValue>) -> Vec<JsonValue> {
  views
    .iter()
    .map(|(view_id, view)| {
      let field_ids = order_ids(view.get("field_orders"));
      let row_ids = order_ids(view.get("row_orders"));

      json!({
        "viewId": view_id,
        "name": view.get("name").cloned().unwrap_or(json!("")),
        "layout": view.get("layout").cloned().unwrap_or(json!(0)),
        "fieldIds": field_ids,
        "rowIds": row_ids,
      })
    })
    .collect()
}

fn order_ids(orders: Option<&JsonValue>) -> Vec<JsonValue> {
  orders
    .and_then(|v| v.as_array())
    .map(|entries| {
      entries
        .iter()
        .filter_map(|entry| entry.get("id").cloned())
        .collect()
    })
    .unwrap_or_default()
}

fn row_summary(row_id: &str, row_json: &JsonValue) -> JsonValue {
  let cells = row_json
    .get("data")
    .and_then(|data| data.get("cells"))
    .cloned()
    .unwrap_or(json!({}));

  json!({
    "rowId": row_id,
    "cells": cells,
  })
}

/// Converts a `DocumentData` block tree into the Slate `Descendant[]` shape
/// AppFlowy-Web's `document-yjs-render-bridge.ts` (`createBlockFromSlateElement`)
/// consumes to build the live editor document. This is the inverse of that
/// function.
///
/// Known limitation: delta ops whose `insert` is not a plain string (e.g.
/// mentions/embeds) are dropped, matching AppFlowy-Web's own
/// `yTextFromSerializedDelta` fallback behavior for invalid deltas.
fn document_data_to_slate_children(document: &DocumentData) -> Vec<JsonValue> {
  let root_children_key = document
    .blocks
    .get(&document.page_id)
    .map(|block| block.children.clone());

  match root_children_key {
    Some(key) => build_slate_children(document, &key),
    None => vec![],
  }
}

fn build_slate_children(document: &DocumentData, children_key: &str) -> Vec<JsonValue> {
  let Some(child_ids) = document.meta.children_map.get(children_key) else {
    return vec![];
  };

  child_ids
    .iter()
    .filter_map(|id| document.blocks.get(id))
    .map(|block| block_to_slate_element(document, block))
    .collect()
}

fn block_to_slate_element(document: &DocumentData, block: &Block) -> JsonValue {
  let mut children: Vec<JsonValue> = Vec::new();

  if block.external_type.as_deref() == Some("text") {
    if let Some(external_id) = &block.external_id {
      let delta = document
        .meta
        .text_map
        .as_ref()
        .and_then(|text_map| text_map.get(external_id));

      children.push(json!({
        "type": "text",
        "textId": external_id,
        "children": delta_to_slate_leaves(delta),
      }));
    }
  }

  children.extend(build_slate_children(document, &block.children));

  json!({
    "type": block.ty,
    "blockId": block.id,
    "relationId": block.children,
    "data": block.data,
    "children": children,
  })
}

fn delta_to_slate_leaves(delta: Option<&String>) -> Vec<JsonValue> {
  let ops: Vec<JsonValue> = delta
    .and_then(|s| serde_json::from_str::<Vec<JsonValue>>(s).ok())
    .unwrap_or_default();

  let mut leaves: Vec<JsonValue> = ops
    .into_iter()
    .filter_map(|op| {
      let insert = op.get("insert")?.as_str()?.to_string();
      let mut leaf = JsonMap::new();
      leaf.insert("text".to_string(), JsonValue::String(insert));
      if let Some(attributes) = op.get("attributes").and_then(|a| a.as_object()) {
        for (k, v) in attributes {
          leaf.insert(k.clone(), v.clone());
        }
      }
      Some(JsonValue::Object(leaf))
    })
    .collect();

  if leaves.is_empty() {
    leaves.push(json!({ "text": "" }));
  }

  leaves
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use collab_document::blocks::DocumentMeta;

  use super::*;

  fn block(id: &str, ty: &str, parent: &str, children: &str, external_id: Option<&str>) -> Block {
    Block {
      id: id.to_string(),
      ty: ty.to_string(),
      parent: parent.to_string(),
      children: children.to_string(),
      external_id: external_id.map(|s| s.to_string()),
      external_type: external_id.map(|_| "text".to_string()),
      data: HashMap::new(),
    }
  }

  #[test]
  fn converts_nested_paragraph_with_formatted_text_to_slate_children() {
    let mut blocks = HashMap::new();
    blocks.insert(
      "page".to_string(),
      block("page", "page", "", "page-children", None),
    );
    blocks.insert(
      "p1".to_string(),
      block("p1", "paragraph", "page", "p1-children", Some("p1-text")),
    );
    blocks.insert(
      "p2".to_string(),
      block("p2", "paragraph", "p1", "p2-children", Some("p2-text")),
    );

    let mut children_map = HashMap::new();
    children_map.insert("page-children".to_string(), vec!["p1".to_string()]);
    children_map.insert("p1-children".to_string(), vec!["p2".to_string()]);
    children_map.insert("p2-children".to_string(), vec![]);

    let mut text_map = HashMap::new();
    text_map.insert(
      "p1-text".to_string(),
      serde_json::to_string(&json!([{"insert": "Hello ", "attributes": {"bold": true}}, {"insert": "world"}]))
        .unwrap(),
    );
    text_map.insert("p2-text".to_string(), serde_json::to_string(&json!([])).unwrap());

    let document = DocumentData {
      page_id: "page".to_string(),
      blocks,
      meta: DocumentMeta {
        children_map,
        text_map: Some(text_map),
      },
    };

    let slate = document_data_to_slate_children(&document);

    assert_eq!(slate.len(), 1);
    let p1 = &slate[0];
    assert_eq!(p1["type"], "paragraph");
    assert_eq!(p1["blockId"], "p1");

    let p1_children = p1["children"].as_array().unwrap();
    // first child is the text node, second is the nested paragraph
    assert_eq!(p1_children[0]["type"], "text");
    let leaves = p1_children[0]["children"].as_array().unwrap();
    assert_eq!(leaves[0]["text"], "Hello ");
    assert_eq!(leaves[0]["bold"], true);
    assert_eq!(leaves[1]["text"], "world");

    let p2 = &p1_children[1];
    assert_eq!(p2["type"], "paragraph");
    assert_eq!(p2["blockId"], "p2");
    let p2_text_leaves = p2["children"].as_array().unwrap()[0]["children"]
      .as_array()
      .unwrap();
    // empty delta falls back to a single empty leaf, matching AppFlowy-Web's
    // own fallback behavior for invalid/empty deltas.
    assert_eq!(p2_text_leaves.len(), 1);
    assert_eq!(p2_text_leaves[0]["text"], "");
  }

  #[test]
  fn drops_non_string_insert_ops_in_delta() {
    let delta = serde_json::to_string(&json!([
      {"insert": "before "},
      {"insert": {"mention": "user-1"}},
      {"insert": "after"},
    ]))
    .unwrap();

    let leaves = delta_to_slate_leaves(Some(&delta));

    assert_eq!(leaves.len(), 2);
    assert_eq!(leaves[0]["text"], "before ");
    assert_eq!(leaves[1]["text"], "after");
  }
}
