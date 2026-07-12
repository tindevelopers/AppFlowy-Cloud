use app_error::AppError;
use database_entity::dto::{AFAccessLevel, AFRole};
use shared_entity::dto::guest_dto::{SharedUser, SharedView, SharedViewDetails};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub async fn insert_shared_view_grants(
  pg_pool: &PgPool,
  workspace_id: Uuid,
  view_id: &str,
  uids: &[i64],
  access_level: AFAccessLevel,
  shared_by: i64,
) -> Result<(), AppError> {
  let access_level_int: i32 = access_level.into();
  for &uid in uids {
    sqlx::query(
      r#"
      INSERT INTO af_shared_view (workspace_id, view_id, uid, access_level, shared_by)
      VALUES ($1, $2, $3, $4, $5)
      ON CONFLICT (workspace_id, view_id, uid)
      DO UPDATE SET access_level = EXCLUDED.access_level, shared_by = EXCLUDED.shared_by
      "#,
    )
    .bind(workspace_id)
    .bind(view_id)
    .bind(uid)
    .bind(access_level_int)
    .bind(shared_by)
    .execute(pg_pool)
    .await?;
  }
  Ok(())
}

pub async fn delete_shared_view_grants(
  pg_pool: &PgPool,
  workspace_id: Uuid,
  view_id: &str,
  uids: &[i64],
) -> Result<(), AppError> {
  for &uid in uids {
    sqlx::query(
      r#"
      DELETE FROM af_shared_view
      WHERE workspace_id = $1 AND view_id = $2 AND uid = $3
      "#,
    )
    .bind(workspace_id)
    .bind(view_id)
    .bind(uid)
    .execute(pg_pool)
    .await?;
  }
  Ok(())
}

pub async fn select_shared_views_for_user(
  pg_pool: &PgPool,
  workspace_id: Uuid,
  uid: i64,
) -> Result<Vec<SharedView>, AppError> {
  let rows = sqlx::query_as::<_, SharedViewRow>(
    r#"
    SELECT view_id, access_level
    FROM af_shared_view
    WHERE workspace_id = $1 AND uid = $2
    "#,
  )
  .bind(workspace_id)
  .bind(uid)
  .fetch_all(pg_pool)
  .await?;

  Ok(rows
    .into_iter()
    .map(|row| SharedView {
      view_id: Uuid::parse_str(&row.view_id).unwrap_or_default(),
      access_level: AFAccessLevel::from(row.access_level),
    })
    .collect())
}

struct SharedViewRow {
  view_id: String,
  access_level: i32,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for SharedViewRow {
  fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    let view_id: String = row.try_get("view_id")?;
    let access_level: i32 = row.try_get("access_level")?;
    Ok(SharedViewRow {
      view_id,
      access_level,
    })
  }
}

pub async fn select_shared_users_for_views(
  pg_pool: &PgPool,
  workspace_id: Uuid,
  view_ids: &[Uuid],
) -> Result<Vec<SharedViewDetails>, AppError> {
  let view_ids_str: Vec<String> = view_ids.iter().map(|v| v.to_string()).collect();
  let rows = sqlx::query_as::<_, SharedUserRow>(
    r#"
    SELECT
      sv.view_id,
      sv.access_level,
      u.uid,
      u.email,
      u.name,
      wm.role_id
    FROM af_shared_view sv
    JOIN af_user u ON u.uid = sv.uid
    LEFT JOIN af_workspace_member wm ON wm.workspace_id = sv.workspace_id AND wm.uid = sv.uid
    WHERE sv.workspace_id = $1 AND sv.view_id = ANY($2)
    "#,
  )
  .bind(workspace_id)
  .bind(&view_ids_str)
  .fetch_all(pg_pool)
  .await?;

  use std::collections::HashMap;
  let mut map: HashMap<Uuid, Vec<SharedUser>> = HashMap::new();
  for row in rows {
    let vid = Uuid::parse_str(&row.view_id).unwrap_or_default();
    let user = SharedUser {
      view_id: vid,
      email: row.email,
      name: row.name.unwrap_or_default(),
      access_level: AFAccessLevel::from(row.access_level),
      role: AFRole::from(row.role_id.unwrap_or(3)),
      avatar_url: None,
      pending_invitation: false,
    };
    map.entry(vid).or_default().push(user);
  }

  Ok(view_ids
    .iter()
    .map(|vid| SharedViewDetails {
      view_id: *vid,
      shared_with: map.get(vid).cloned().unwrap_or_default(),
    })
    .collect())
}

struct SharedUserRow {
  view_id: String,
  access_level: i32,
  email: String,
  name: Option<String>,
  role_id: Option<i32>,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for SharedUserRow {
  fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
    Ok(SharedUserRow {
      view_id: row.try_get("view_id")?,
      access_level: row.try_get("access_level")?,
      email: row.try_get("email")?,
      name: row.try_get("name")?,
      role_id: row.try_get("role_id")?,
    })
  }
}
