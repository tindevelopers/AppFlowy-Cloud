// Collab-sync: in-place AppFlowy document updates with section markers.
//
// Modes:
//   collab-sync list                    — print page hierarchy with view IDs
//   collab-sync fetch <view_id>         — dump remote block tree
//   collab-sync convert [file.md]       — run MDImporter locally, print block tree
//   collab-sync seed <view_id> <file.md> — full write (initial page seed)
//   collab-sync sync <view_id> <file.md> --section <key> [--dry-run]
//                                       — replace marked section in existing page
//   collab-sync sync --all --manifest <path> [--dry-run]
//                                       — sync all sections across all pages
//
// Section-marked content:
//   <!-- BEGIN GENERATED:<key> -->
//   ...content...
//   <!-- END GENERATED:<key> -->
//
// Only blocks between matching markers are replaced. Everything outside is
// left untouched. Missing markers → error (never blanket-overwrites).

use anyhow::{Context, Result};
use client_api::entity::{CreateCollabParams, QueryCollab, QueryCollabParams};
use client_api::{Client, ClientConfiguration};
use collab::core::collab::DataSource;
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_document::blocks::{DocumentData, DocumentMeta};
use collab_document::document::Document;
use collab_document::importer::md_importer::MDImporter;
use collab_entity::CollabType;
use std::collections::HashMap;
use uuid::Uuid;

const WORKSPACE_ID: &str = "d7cc1938-4ed2-4b8d-a9b6-4cec3c3bc046";
const DEFAULT_VIEW_ID: &str = "0c0b2e92-c9f7-4021-a6e3-d306e160490e"; // Flutter Frontend

const SAMPLE_MD: &str = r#"# Test Section

This paragraph has **bold** and `inline code`.

<!-- BEGIN GENERATED:sample -->
* Item one
* Item two with _italic_
<!-- END GENERATED:sample -->

Another paragraph **outside** the markers.
"#;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env")).ok();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("fetch");

    match mode {
        "list" => {
            let ws = args.get(2).map(|s| s.as_str()).unwrap_or(WORKSPACE_ID);
            cmd_list(ws).await?;
        }
        "convert" => {
            let path = args.get(2).map(|s| s.to_string());
            cmd_convert(path)?;
        }
        "seed" => {
            let view_id = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| DEFAULT_VIEW_ID.to_string());
            let md_path = args
                .get(3)
                .cloned()
                .context("usage: collab-sync seed <view_id> <file.md> [--workspace <id>]")?;
            let ws = parse_flag(&args, "--workspace")
                .unwrap_or_else(|| WORKSPACE_ID.to_string());
            cmd_seed(&ws, &view_id, &md_path).await?;
        }
        "sync" => {
            let ws = parse_flag(&args, "--workspace")
                .unwrap_or_else(|| WORKSPACE_ID.to_string());
            let all = args.iter().any(|a| a == "--all");
            if all {
                let manifest_path = parse_flag(&args, "--manifest")
                    .unwrap_or_else(|| "manifest.json".to_string());
                let dry_run = args.iter().any(|a| a == "--dry-run");
                let backup = args.iter().any(|a| a == "--backup");
                cmd_sync_all(&ws, &manifest_path, dry_run, backup).await?;
            } else {
                let view_id = args
                    .get(2)
                    .cloned()
                    .unwrap_or_else(|| DEFAULT_VIEW_ID.to_string());
                let md_path = args
                    .get(3)
                    .cloned()
                    .context("usage: collab-sync sync <view_id> <file.md> [--section <key>] [--dry-run] [--backup] [--workspace <id>]")?;
                let section_key = parse_flag(&args, "--section").unwrap_or_else(|| "main".to_string());
                let dry_run = args.iter().any(|a| a == "--dry-run");
                let backup = args.iter().any(|a| a == "--backup");
                cmd_sync(&ws, &view_id, &md_path, &section_key, dry_run, backup).await?;
            }
        }
        "restore" => {
            let view_id = args
                .get(2)
                .cloned()
                .context("usage: collab-sync restore <view_id> <backup.bin> [--workspace <id>]")?;
            let backup_path = args
                .get(3)
                .cloned()
                .context("usage: collab-sync restore <view_id> <backup.bin>")?;
            let ws = parse_flag(&args, "--workspace")
                .unwrap_or_else(|| WORKSPACE_ID.to_string());
            cmd_restore(&ws, &view_id, &backup_path).await?;
        }
        _ => {
            let view_id = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| DEFAULT_VIEW_ID.to_string());
            let ws = parse_flag(&args, "--workspace")
                .unwrap_or_else(|| WORKSPACE_ID.to_string());
            cmd_fetch(&ws, &view_id).await?;
        }
    }

    Ok(())
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    let idx = args.iter().position(|a| a == flag)?;
    args.get(idx + 1).cloned()
}

// ── list ─────────────────────────────────────────────────────────────────────

async fn cmd_list(workspace_id_str: &str) -> Result<()> {
    let client = connect_and_auth().await?;
    let workspace_id: Uuid = workspace_id_str.parse()?;
    let folder = client
        .get_workspace_folder(&workspace_id, Some(10), None)
        .await
        .context("get_workspace_folder failed")?;
    print_folder(&folder, 0);
    Ok(())
}

fn print_folder(view: &client_api_entity::workspace_dto::FolderView, depth: usize) {
    let indent = "  ".repeat(depth);
    let space = if view.is_space { " [SPACE]" } else { "" };
    println!(
        "{}  {}{}{}",
        view.view_id, indent, view.name, space
    );
    for child in &view.children {
        print_folder(child, depth + 1);
    }
}

// ── seed (full write, no markers) ────────────────────────────────────────────

async fn cmd_seed(workspace_id_str: &str, view_id: &str, md_path: &str) -> Result<()> {
    let client = connect_and_auth().await?;
    let workspace_id: Uuid = workspace_id_str.parse()?;
    let object_id: Uuid = view_id.parse()?;

    let backup = std::env::args().any(|a| a == "--backup");
    if backup {
        save_backup(&client, workspace_id, object_id).await?;
    }

    let md_content = std::fs::read_to_string(md_path).context("reading markdown file")?;
    let data = build_replacement_document_data(&md_content)?;

    println!("seed {} blocks to view {}", data.blocks.len(), view_id);

    let doc = Document::create(&object_id.to_string(), data)
        .context("Document::create failed")?;
    let encoded = doc.encode_collab().context("encode_collab failed")?;

    upload_collab(&client, workspace_id, object_id, encoded).await?;
    println!("[ok] seeded.");

    Ok(())
}

// ── convert ──────────────────────────────────────────────────────────────────

fn cmd_convert(path: Option<String>) -> Result<()> {
    let md = match path {
        Some(ref p) => std::fs::read_to_string(p).context("reading markdown file")?,
        None => SAMPLE_MD.to_string(),
    };

    let doc_id = Uuid::new_v4().to_string();
    let importer = MDImporter::new(None);
    let data = importer
        .import(&doc_id, md)
        .context("MDImporter::import failed")?;

    let text_map: HashMap<String, String> = data.meta.text_map.clone().unwrap_or_default();
    println!(
        "[document] page_id={} blocks={} children_groups={} text_entries={}\n",
        data.page_id, data.blocks.len(), data.meta.children_map.len(), text_map.len()
    );
    println!("--- block tree ---");
    walk(&data.page_id, &data, &text_map, 0);

    println!("\n--- marker scan ---");
    let markers = find_markers(&data, &text_map);
    if markers.is_empty() {
        println!("(no BEGIN/END GENERATED markers found)");
    } else {
        for (key, begin, end) in &markers {
            println!("  section={} BEGIN={} END={}", key, begin, end);
        }
        println!("[ok] Found {} section(s).", markers.len());
    }

    Ok(())
}

// ── fetch ────────────────────────────────────────────────────────────────────

async fn cmd_fetch(workspace_id_str: &str, view_id: &str) -> Result<()> {
    let client = connect_and_auth().await?;
    let workspace_id: Uuid = workspace_id_str.parse()?;
    let object_id: Uuid = view_id.parse()?;

    println!("workspace  = {}", workspace_id);
    println!("view/doc   = {}", object_id);

    let data = fetch_document(&client, workspace_id, object_id).await?;
    let text_map: HashMap<String, String> = data.meta.text_map.clone().unwrap_or_default();

    println!(
        "\n[document] page_id={} blocks={} children_groups={} text_entries={}",
        data.page_id, data.blocks.len(), data.meta.children_map.len(), text_map.len()
    );
    println!("\n--- block tree ---");
    walk(&data.page_id, &data, &text_map, 0);

    println!("\n--- marker scan ---");
    let markers = find_markers(&data, &text_map);
    if markers.is_empty() {
        println!("(no BEGIN/END GENERATED markers present)");
    } else {
        for (key, begin, end) in &markers {
            println!("  section={} BEGIN={} END={}", key, begin, end);
        }
    }

    Ok(())
}

// ── sync ─────────────────────────────────────────────────────────────────────

async fn cmd_sync(workspace_id_str: &str, view_id: &str, md_path: &str, section_key: &str, dry_run: bool, backup: bool) -> Result<()> {
    let client = connect_and_auth().await?;
    let workspace_id: Uuid = workspace_id_str.parse()?;
    let object_id: Uuid = view_id.parse().context("view id must be a UUID")?;

    println!("section   = {}", section_key);
    println!("markdown  = {}", md_path);
    println!("view/doc  = {}", object_id);
    if dry_run {
        println!("mode      = DRY RUN (no write)");
    }

    if backup && !dry_run {
        save_backup(&client, workspace_id, object_id).await?;
    }

    let md_content =
        std::fs::read_to_string(md_path).context("reading markdown file")?;

    let data = fetch_document(&client, workspace_id, object_id).await?;
    let text_map: HashMap<String, String> = data.meta.text_map.clone().unwrap_or_default();

    let markers = find_markers(&data, &text_map);
    let marker_key = format!("BEGIN GENERATED:{}", section_key);
    let end_key = format!("END GENERATED:{}", section_key);

    let target = markers
        .iter()
        .find(|(k, _, _)| k == &marker_key);

    let (begin_id, end_id) = match target {
        Some((_, b, e)) => (b.clone(), e.clone()),
        None => {
            let available: Vec<&str> = markers.iter().map(|(k, _, _)| {
                k.strip_prefix("BEGIN GENERATED:").unwrap_or(k)
            }).collect();
            anyhow::bail!(
                "section '{}' not found on this page. Available sections: {:?}",
                section_key, available
            );
        }
    };

    println!(
        "[ok] found section '{}': BEGIN={} END={}",
        section_key, begin_id, end_id
    );

    let region_blocks = collect_region_blocks(&data, &begin_id, &end_id)?;
    println!(
        "[ok] region contains {} blocks (will be replaced)",
        region_blocks.len()
    );

    if dry_run {
        println!("\n--- current region content ---");
        for bid in &region_blocks {
            if let Some(block) = data.blocks.get(bid) {
                let snippet = block
                    .external_id
                    .as_ref()
                    .and_then(|ext| text_map.get(ext))
                    .map(|t| t.replace('\n', " "))
                    .unwrap_or_default();
                println!("  [{}] {} {}", block.ty, bid, snippet);
            }
        }

        let new_data = build_replacement_document_data(&md_content)?;
        let new_text_map: HashMap<String, String> =
            new_data.meta.text_map.clone().unwrap_or_default();
        println!("\n--- would write ---");
        if let Some(children) = new_data.meta.children_map.get(&new_data.page_id) {
            for child in children {
                walk(child, &new_data, &new_text_map, 1);
            }
        }
        println!("\n(new blocks: {})", new_data.blocks.len());
        return Ok(());
    }

    let new_data = build_replacement_document_data(&md_content)?;
    let modified = merge_replacement(&data, &begin_id, &end_id, new_data, &marker_key, &end_key)?;

    println!(
        "[ok] modified doc: {} blocks total",
        modified.blocks.len()
    );

    let doc = Document::create(&object_id.to_string(), modified)
        .context("Document::create failed")?;
    let encoded = doc.encode_collab().context("encode_collab failed")?;

    upload_collab(&client, workspace_id, object_id, encoded).await?;
    println!("[ok] uploaded.");

    Ok(())
}

// ── sync --all ───────────────────────────────────────────────────────────────

async fn cmd_sync_all(workspace_id_str: &str, manifest_path: &str, dry_run: bool, backup: bool) -> Result<()> {
    let manifest_raw = std::fs::read_to_string(manifest_path)
        .context("reading manifest file")?;
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).context("parsing manifest JSON")?;

    let pages = manifest["pages"]
        .as_object()
        .context("manifest missing 'pages' key")?;

    let client = connect_and_auth().await?;
    let workspace_id: Uuid = workspace_id_str.parse()?;

    for (name, page) in pages {
        let view_id = page["view_id"].as_str().context("page missing view_id")?;
        let source = page["source_md"].as_str().context("page missing source_md")?;
        let sections = page["sections"]
            .as_array()
            .context("page missing sections")?;

        println!("--- {} ({}) ---", name, view_id);
        let object_id: Uuid = view_id.parse()?;

        let md_path = source.to_string();

        if backup && !dry_run {
            save_backup(&client, workspace_id, object_id).await?;
        }

        for section in sections {
            let section_key = section.as_str().unwrap_or("main");
            println!("  section={}", section_key);

            let md_content =
                std::fs::read_to_string(&md_path).context("reading markdown file")?;

            let data = fetch_document(&client, workspace_id, object_id).await?;
            let text_map: HashMap<String, String> =
                data.meta.text_map.clone().unwrap_or_default();
            let markers = find_markers(&data, &text_map);
            let marker_key = format!("BEGIN GENERATED:{}", section_key);

            let target = markers.iter().find(|(k, _, _)| k == &marker_key);
            let (begin_id, end_id) = match target {
                Some((_, b, e)) => (b.clone(), e.clone()),
                None => {
                    eprintln!("  WARN: section '{}' not found on '{}', skipping", section_key, name);
                    continue;
                }
            };

            let region_blocks = collect_region_blocks(&data, &begin_id, &end_id)?;
            println!("  replacing {} blocks", region_blocks.len());

            if dry_run {
                println!("  DRY RUN: would replace {} blocks", region_blocks.len());
                continue;
            }

            let new_data = build_replacement_document_data(&md_content)?;
            let modified =
                merge_replacement(&data, &begin_id, &end_id, new_data, "", "")?;

            let doc = Document::create(&object_id.to_string(), modified)?;
            let encoded = doc.encode_collab()?;
            upload_collab(&client, workspace_id, object_id, encoded).await?;
            println!("  [ok] uploaded");
        }
    }

    println!("\n[done] All pages synced.");
    Ok(())
}

// ── restore ──────────────────────────────────────────────────────────────────

async fn cmd_restore(workspace_id_str: &str, view_id: &str, backup_path: &str) -> Result<()> {
    let client = connect_and_auth().await?;
    let workspace_id: Uuid = workspace_id_str.parse()?;
    let object_id: Uuid = view_id.parse()?;

    let doc_state = std::fs::read(backup_path).context("reading backup file")?;

    let collab = Collab::new_with_source(
        CollabOrigin::Empty,
        &object_id.to_string(),
        DataSource::DocStateV1(doc_state),
        vec![],
        false,
    )?;
    let document = Document::open(collab).context("opening backup collab")?;
    let encoded = document.encode_collab().context("encoding backup")?;

    upload_collab(&client, workspace_id, object_id, encoded).await?;
    println!("[ok] restored from {}", backup_path);

    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn find_markers(
    data: &DocumentData,
    text_map: &HashMap<String, String>,
) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    for (block_id, block) in data.blocks.iter() {
        let empty = String::new();
        let delta = block
            .external_id
            .as_ref()
            .and_then(|ext| text_map.get(ext))
            .unwrap_or(&empty);
        // Markers live inside JSON delta strings like:
        //   [{"insert":"<!-- BEGIN GENERATED:key -->"}]
        let begin_prefix = "<!-- BEGIN GENERATED:";
        if delta.contains(begin_prefix) {
            if let Some(key_start) = delta.find(begin_prefix) {
                let after_prefix = &delta[key_start + begin_prefix.len()..];
                if let Some(key_end) = after_prefix.find(" -->") {
                    let section_key = after_prefix[..key_end].to_string();
                    let begin_id = block_id.clone();
                    let end_id =
                        find_next_end_marker(data, text_map, block_id, &section_key);
                    if let Some(eid) = end_id {
                        result.push((
                            format!("BEGIN GENERATED:{}", section_key),
                            begin_id,
                            eid,
                        ));
                    }
                }
            }
        }
    }
    result
}

fn find_next_end_marker(
    data: &DocumentData,
    text_map: &HashMap<String, String>,
    after_id: &str,
    section_key: &str,
) -> Option<String> {
    let page_id = &data.page_id;
    let children = data.meta.children_map.get(page_id)?;
    let target = format!("<!-- END GENERATED:{} -->", section_key);
    let mut found_start = false;
    for child_id in children {
        if child_id == after_id {
            found_start = true;
            continue;
        }
        if !found_start {
            continue;
        }
        let block = data.blocks.get(child_id)?;
        let empty = String::new();
        let delta = block
            .external_id
            .as_ref()
            .and_then(|ext| text_map.get(ext))
            .unwrap_or(&empty);
        if delta.contains(&target) {
            return Some(child_id.clone());
        }
    }
    None
}

fn collect_region_blocks(
    data: &DocumentData,
    begin_id: &str,
    end_id: &str,
) -> Result<Vec<String>> {
    let page_id = &data.page_id;
    let children = data
        .meta
        .children_map
        .get(page_id)
        .context("page has no children")?;

    let begin_pos = children.iter().position(|c| c == begin_id)
        .context("BEGIN marker not in children")?;
    let end_pos = children.iter().position(|c| c == end_id)
        .context("END marker not in children")?;

    if end_pos <= begin_pos + 1 {
        return Ok(vec![]); // empty region
    }

    Ok(children[begin_pos + 1..end_pos].to_vec())
}

fn build_replacement_document_data(md: &str) -> Result<DocumentData> {
    let doc_id = Uuid::new_v4().to_string();
    let importer = MDImporter::new(None);
    let full_data = importer.import(&doc_id, md.to_string())?;
    Ok(full_data)
}

/// Merge replacement blocks into the existing document data, replacing the
/// region between `begin_id` and `end_id` with the new blocks.
fn merge_replacement(
    original: &DocumentData,
    begin_id: &str,
    end_id: &str,
    new_data: DocumentData,
    _marker_begin_key: &str,
    _marker_end_key: &str,
) -> Result<DocumentData> {
    let page_id = original.page_id.clone();
    let mut blocks = original.blocks.clone();
    let mut children_map = original.meta.children_map.clone();
    let mut text_map = original.meta.text_map.clone().unwrap_or_default();

    let page_children = children_map
        .get(&page_id)
        .cloned()
        .context("page has no children")?;

    let begin_pos = page_children
        .iter()
        .position(|c| c == begin_id)
        .context("BEGIN marker not found")?;
    let end_pos = page_children
        .iter()
        .position(|c| c == end_id)
        .context("END marker not found")?;

    let region_ids: Vec<String> =
        page_children[begin_pos + 1..end_pos].to_vec();

    for rid in &region_ids {
        blocks.remove(rid);
        children_map.remove(rid);
        text_map.remove(rid);
    }

    let new_page_id = new_data.page_id;
    let new_page_children = new_data
        .meta
        .children_map
        .get(&new_page_id)
        .cloned()
        .unwrap_or_default();

    let mut merged_children = page_children[..=begin_pos].to_vec();
    for child_id in &new_page_children {
        merged_children.push(child_id.clone());
        if let Some(block) = new_data.blocks.get(child_id) {
            blocks.insert(child_id.clone(), block.clone());
        }
        if let Some(ext_id) = new_data.blocks.get(child_id).and_then(|b| b.external_id.clone()) {
            if let Some(delta) = new_data.meta.text_map.as_ref().and_then(|m| m.get(&ext_id)) {
                text_map.insert(ext_id, delta.clone());
            }
        }
    }
    merged_children.extend(page_children[end_pos..].iter().cloned());
    children_map.insert(page_id.clone(), merged_children);

    for (child_id, grand_children) in new_data.meta.children_map.iter() {
        if child_id != &new_page_id {
            children_map
                .entry(child_id.clone())
                .or_default()
                .extend(grand_children.iter().cloned());
        }
    }

    if let Some(new_text_map) = &new_data.meta.text_map {
        for (ext_id, delta) in new_text_map {
            text_map.entry(ext_id.clone()).or_insert_with(|| delta.clone());
        }
    }

    Ok(DocumentData {
        page_id,
        blocks,
        meta: DocumentMeta {
            children_map,
            text_map: Some(text_map),
        },
    })
}

// ── upload / backup ──────────────────────────────────────────────────────────

async fn upload_collab(
    client: &Client,
    workspace_id: Uuid,
    object_id: Uuid,
    encoded: collab::entity::EncodedCollab,
) -> Result<()> {
    let params = CreateCollabParams {
        workspace_id,
        object_id,
        encoded_collab_v1: encoded
            .encode_to_bytes()
            .context("encode_to_bytes failed")?,
        collab_type: CollabType::Document,
    };
    client
        .create_collab(params)
        .await
        .context("create_collab failed")?;
    Ok(())
}

async fn save_backup(client: &Client, workspace_id: Uuid, object_id: Uuid) -> Result<()> {
    let params = QueryCollabParams {
        workspace_id,
        inner: QueryCollab::new(object_id, CollabType::Document),
    };
    let doc_state = client
        .get_collab(params)
        .await
        .context("get_collab for backup failed")?
        .encode_collab
        .doc_state
        .to_vec();

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let backup_dir = root.join("artifacts").join("backups");
    std::fs::create_dir_all(&backup_dir).context("creating backup dir")?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let name = format!("{}-{}.doc_state", object_id, ts);
    let path = backup_dir.join(&name);
    std::fs::write(&path, &doc_state).context("writing backup")?;
    println!("[backup] saved {} ({} bytes)", name, doc_state.len());

    Ok(())
}

// ── auth / fetch ─────────────────────────────────────────────────────────────

async fn connect_and_auth() -> Result<Client> {
    let email = std::env::var("APPFLOWY_EMAIL").context("APPFLOWY_EMAIL missing")?;
    let password = std::env::var("APPFLOWY_PASSWORD").context("APPFLOWY_PASSWORD missing")?;
    let base = std::env::var("APPFLOWY_BASE_URL")
        .unwrap_or_else(|_| "https://projects.tinconnect.com".to_string());
    let base = base.trim_end_matches('/').to_string();

    let gotrue_url = format!("{}/gotrue", base);
    let ws_base_url = format!("{}/ws/v1", base.replacen("https://", "wss://", 1));
    let device_id = Uuid::new_v4().to_string();

    let client = Client::new(
        &base,
        &ws_base_url,
        &gotrue_url,
        &device_id,
        ClientConfiguration::default(),
        "0.9.4",
    );

    client
        .sign_in_password(&email, &password)
        .await
        .context("GoTrue sign-in failed")?;
    println!("[ok] signed in");
    Ok(client)
}

async fn fetch_document(
    client: &Client,
    workspace_id: Uuid,
    object_id: Uuid,
) -> Result<DocumentData> {
    let params = QueryCollabParams {
        workspace_id,
        inner: QueryCollab::new(object_id, CollabType::Document),
    };
    let doc_state = client
        .get_collab(params)
        .await
        .context("get_collab failed")?
        .encode_collab
        .doc_state
        .to_vec();
    println!("[ok] fetched doc_state: {} bytes", doc_state.len());

    let collab = Collab::new_with_source(
        CollabOrigin::Empty,
        &object_id.to_string(),
        DataSource::DocStateV1(doc_state),
        vec![],
        false,
    )
    .context("rebuild collab from doc_state failed")?;

    let document = Document::open(collab).context("Document::open failed")?;
    document.get_document_data().context("get_document_data failed")
}

// ── block tree walk ──────────────────────────────────────────────────────────

fn walk(
    id: &str,
    data: &DocumentData,
    text_map: &HashMap<String, String>,
    depth: usize,
) {
    let indent = "  ".repeat(depth);
    if let Some(block) = data.blocks.get(id) {
        let snippet = block
            .external_id
            .as_ref()
            .and_then(|ext| text_map.get(ext))
            .map(|t| {
                let t = t.replace('\n', " ");
                if t.len() > 80 { format!("{}…", &t[..80]) } else { t }
            })
            .unwrap_or_default();
        println!("{}[{}] {} {}", indent, block.ty, id, snippet);
    }
    if let Some(children) = data.meta.children_map.get(id) {
        for child in children {
            walk(child, data, text_map, depth + 1);
        }
    }
}
