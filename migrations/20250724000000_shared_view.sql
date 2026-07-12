-- Per-view guest sharing grants
CREATE TABLE IF NOT EXISTS af_shared_view (
  workspace_id UUID NOT NULL REFERENCES af_workspace(workspace_id) ON DELETE CASCADE,
  view_id TEXT NOT NULL,
  uid BIGINT NOT NULL REFERENCES af_user(uid) ON DELETE CASCADE,
  access_level INT NOT NULL DEFAULT 10,
  shared_by BIGINT NOT NULL REFERENCES af_user(uid),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (workspace_id, view_id, uid)
);

CREATE INDEX IF NOT EXISTS idx_af_shared_view_uid ON af_shared_view (uid);
CREATE INDEX IF NOT EXISTS idx_af_shared_view_view_id ON af_shared_view (workspace_id, view_id);
