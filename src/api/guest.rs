use std::collections::HashMap;

use actix_web::{
  web::{self, Data, Json},
  Result, Scope,
};
use access_control::act::Action;
use app_error::{AppError, ErrorCode};
use database::shared_view;
use database::user::select_uid_from_email;
use database::workspace::{select_user_role, upsert_workspace_member_with_txn};
use database_entity::dto::AFRole;
use shared_entity::{
  dto::guest_dto::{
    RevokeSharedViewAccessRequest, ShareViewWithGuestRequest, SharedViewDetails,
    SharedViewDetailsRequest, SharedViews,
  },
  response::{AppResponse, AppResponseError, JsonAppResponse},
};
use uuid::Uuid;

use crate::biz::authentication::jwt::UserUuid;
use crate::state::AppState;

pub fn sharing_scope() -> Scope {
  web::scope("/api/sharing/workspace")
    .service(
      web::resource("{workspace_id}/view")
        .route(web::get().to(list_shared_views_handler))
        .route(web::put().to(put_shared_view_handler)),
    )
    .service(
      web::resource("{workspace_id}/view/{view_id}/access-details")
        .route(web::post().to(shared_view_access_details_handler)),
    )
    .service(
      web::resource("{workspace_id}/view/{view_id}/revoke-access")
        .route(web::post().to(revoke_shared_view_access_handler)),
    )
}

async fn list_shared_views_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  path: web::Path<Uuid>,
) -> Result<JsonAppResponse<SharedViews>> {
  let workspace_id = path.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Read)
    .await?;
  let shared_views =
    shared_view::select_shared_views_for_user(&state.pg_pool, workspace_id, uid).await?;
  Ok(Json(
    AppResponse::Ok().with_data(SharedViews {
      shared_views,
      view_id_with_no_access: vec![],
    }),
  ))
}

async fn put_shared_view_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  payload: web::Json<ShareViewWithGuestRequest>,
  path: web::Path<Uuid>,
) -> Result<JsonAppResponse<()>> {
  let workspace_id = path.into_inner();
  let req = payload.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;

  state
    .workspace_access_control
    .enforce_role_weak(&uid, &workspace_id, AFRole::Member)
    .await?;

  let mut email_to_uid: HashMap<String, i64> = HashMap::new();
  for email in &req.emails {
    match select_uid_from_email(&state.pg_pool, email).await {
      Ok(target_uid) => {
        email_to_uid.insert(email.clone(), target_uid);
      },
      Err(_) => {
        tracing::warn!("User not found for email: {}, skipping", email);
      },
    }
  }

  if email_to_uid.is_empty() {
    return Err(AppResponseError::new(
      ErrorCode::InvalidRequest,
      "No registered users found for the provided emails. Users must create an account first.",
    )
    .into());
  }

  let uids: Vec<i64> = email_to_uid.values().copied().collect();

  for (email, &target_uid) in &email_to_uid {
    let existing_role = select_user_role(&state.pg_pool, &target_uid, &workspace_id).await;
    if existing_role.is_err() {
      let mut txn = state
        .pg_pool
        .begin()
        .await
        .map_err(|e| AppError::Unhandled(e.to_string()))?;
      upsert_workspace_member_with_txn(&mut txn, &workspace_id, email, AFRole::Guest).await?;
      txn
        .commit()
        .await
        .map_err(|e| AppError::Unhandled(e.to_string()))?;
    }
  }

  shared_view::insert_shared_view_grants(
    &state.pg_pool,
    workspace_id,
    &req.view_id.to_string(),
    &uids,
    req.access_level,
    uid,
  )
  .await?;

  Ok(Json(AppResponse::Ok()))
}

async fn shared_view_access_details_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  json: Json<SharedViewDetailsRequest>,
  path: web::Path<(Uuid, Uuid)>,
) -> Result<JsonAppResponse<SharedViewDetails>> {
  let (workspace_id, view_id) = path.into_inner();
  let req = json.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_action(&uid, &workspace_id, Action::Read)
    .await?;

  let mut all_view_ids: Vec<Uuid> = req.ancestor_view_ids.clone();
  if !all_view_ids.contains(&view_id) {
    all_view_ids.push(view_id);
  }

  let details_list =
    shared_view::select_shared_users_for_views(&state.pg_pool, workspace_id, &all_view_ids)
      .await?;

  let primary = details_list
    .iter()
    .find(|d| d.view_id == view_id)
    .cloned()
    .unwrap_or(SharedViewDetails {
      view_id,
      shared_with: vec![],
    });

  Ok(Json(AppResponse::Ok().with_data(primary)))
}

async fn revoke_shared_view_access_handler(
  user_uuid: UserUuid,
  state: Data<AppState>,
  payload: web::Json<RevokeSharedViewAccessRequest>,
  path: web::Path<(Uuid, Uuid)>,
) -> Result<JsonAppResponse<()>> {
  let (workspace_id, view_id) = path.into_inner();
  let req = payload.into_inner();
  let uid = state.user_cache.get_user_uid(&user_uuid).await?;
  state
    .workspace_access_control
    .enforce_role_weak(&uid, &workspace_id, AFRole::Member)
    .await?;

  let mut uids = Vec::new();
  for email in &req.emails {
    if let Ok(target_uid) = select_uid_from_email(&state.pg_pool, email).await {
      uids.push(target_uid);
    }
  }

  shared_view::delete_shared_view_grants(
    &state.pg_pool,
    workspace_id,
    &view_id.to_string(),
    &uids,
  )
  .await?;

  Ok(Json(AppResponse::Ok()))
}
