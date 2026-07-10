'use strict';

const Y = require('yjs');

function toUint8(bytes) {
  if (bytes instanceof Uint8Array) {
    return bytes.buffer === bytes.buffer
      ? new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
      : bytes;
  }
  if (Array.isArray(bytes)) return Uint8Array.from(bytes);
  throw new Error(`unsupported doc_state type: ${typeof bytes}`);
}

function decodeFolderCollab(docStateBytes) {
  const doc = new Y.Doc();
  Y.applyUpdate(doc, toUint8(docStateBytes));
  const data = doc.getMap('data').toJSON();
  return data && data.folder ? data.folder : null;
}

function projectFolder(workspaceId, folder) {
  const views    = (folder && folder.views)    || {};
  const relation = (folder && folder.relation) || {};

  const prevOf = new Map();
  for (const [, kids] of Object.entries(relation)) {
    let prev = null;
    for (const k of kids) { prevOf.set(k.id, prev); prev = k.id; }
  }

  const rows = [];
  for (const [vid, v] of Object.entries(views)) {
    const parent = v.bid && v.bid.length ? v.bid : null;
    let extraObj = null;
    if (v.extra) { try { extraObj = JSON.parse(v.extra); } catch (_) {} }
    let iconObj = null;
    if (v.icon && v.icon.length) { try { iconObj = JSON.parse(v.icon); } catch (_) {} }

    rows.push({
      workspace_id: workspaceId,
      view_id: vid,
      parent_view_id: parent,
      name: v.name || '',
      icon: iconObj,
      layout: parseInt(v.layout || '0', 10),
      is_space: !!(extraObj && extraObj.is_space),
      extra: extraObj,
      created_at: Number(v.created_at || 0),
      created_by: v.created_by ? Number(v.created_by) : null,
      last_edited_time: Number(v.last_edited_time || v.created_at || 0),
      last_edited_by: v.last_edited_by ? Number(v.last_edited_by) : null,
      prev_view_id: prevOf.get(vid) || null,
      children: (relation[vid] || []).map(k => k.id),
    });
  }
  return rows;
}

async function fetchFolderViaApi(apiBase, token, workspaceId) {
  const url = `${apiBase.replace(/\/+$/, '')}/api/workspace/v1/${workspaceId}/collab/${workspaceId}?collab_type=3`;
  const res = await fetch(url, { headers: { Authorization: `Bearer ${token}` } });
  if (!res.ok) throw new Error(`API ${res.status} for ${workspaceId}`);
  const j = await res.json();
  if (!j.data || !j.data.doc_state) throw new Error(`missing doc_state for ${workspaceId}`);
  return decodeFolderCollab(j.data.doc_state);
}

module.exports = { decodeFolderCollab, projectFolder, fetchFolderViaApi };
