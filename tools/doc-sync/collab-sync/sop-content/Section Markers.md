# Section Markers

<!-- BEGIN GENERATED:main -->

Section markers let the collab-sync tool update generated content while preserving human edits on the same page.

## Marker Format

```
&lt;!-- BEGIN GENERATED:&lt;key&gt; --&gt;
...this content is replaced during sync...
&lt;!-- END GENERATED:&lt;key&gt; --&gt;
```

- Each marker pair has a **key** that identifies the section (e.g., `main`, `sidebar`, `api-reference`).
- Multiple sections on one page must use different keys.
- Markers must be on their own line as top-level paragraph blocks.
- Everything between matching markers is replaced atomically.

## How It Works

1. `sync` fetches the page's current block tree from the server.
2. It scans for paragraph blocks containing `&lt;!-- BEGIN GENERATED:&lt;key&gt; --&gt;` and `&lt;!-- END GENERATED:&lt;key&gt; --&gt;`.
3. All sibling blocks between the two markers are identified as the **replaceable region**.
4. The fresh markdown is converted to blocks via `MDImporter`.
5. The old region is removed from the block tree and the new blocks are inserted in its place.
6. The modified document is encoded and uploaded to the server.

## What Gets Preserved

- **Content outside markers** — any block before BEGIN or after END is never read or written.
- **Page identity** — the view ID and page ID never change.
- **Sidebar structure** — the page stays in the same position in the workspace tree.
- **Comments/reactions** — anchored to blocks outside the markers, not affected by sync.

## Marker Safety

- **Missing markers** — `sync` refuses to write and reports which sections are available (never blanket-overwrites).
- **Empty region** — if no content exists between markers, the new content is simply inserted there.
- **Nested markers** — not supported. Keep markers at the top level of a page's block tree.

## Adding Markers to Existing Pages

Use the `add-markers.py` script to batch-add markers to markdown files:

```bash
cd appflowy-agent-tools/collab-sync
python3 add-markers.py
```

Then re-seed the pages with `cargo run -- seed <view_id> <file.md>`.

## Adding Markers Manually

Open the page in the AppFlowy editor and insert these exact lines as separate paragraphs:

```
&lt;!-- BEGIN GENERATED:my-section --&gt;
(your content here)
&lt;!-- END GENERATED:my-section --&gt;
```

The tool will detect them on the next sync.
<!-- END GENERATED:main -->
