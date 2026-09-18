//! Redeeming a voucher — the one way to gain subscription time outside
//! `/api/billing/checkout`.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::extract::{AppJson, CurrentUser};
use crate::domain::billing::Subscription;
use crate::error::AppResult;
use crate::services::voucher;
use crate::state::AppState;

#[derive(Deserialize, ToSchema)]
pub struct RedeemRequest {
    pub code: String,
}

#[derive(Serialize, ToSchema)]
pub struct RedeemResponse {
    pub subscription: Subscription,
}

/// `POST /api/subscribe/redeem`
///
/// Any signed-in account may attempt this — existing access is not a
/// precondition, since redeeming is how an account with none gets some.
#[utoipa::path(
    post, path = "/api/subscribe/redeem", tag = "billing",
    summary = "Redeem a voucher code",
    description = "Extends the account's access by the voucher's duration, starting from the later of now and the account's current paid-through date. Rate limited per account.",
    security(("session" = [])),
    request_body = RedeemRequest,
    responses(
        (status = 200, description = "The account's updated subscription", body = RedeemResponse),
        (status = 400, description = "The code does not exist, is inactive, expired, or already used up", body = crate::error::ErrorBody),
        (status = 409, description = "This account has already redeemed this code", body = crate::error::ErrorBody),
        (status = 429, description = "Too many attempts — try again shortly", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn redeem(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppJson(body): AppJson<RedeemRequest>,
) -> AppResult<Json<RedeemResponse>> {
    let subscription = voucher::redeem(
        &state.db,
        &state.config.billing,
        &state.redeem_rate_limiter,
        user.id,
        body.code.trim(),
    )
    .await?;

    Ok(Json(RedeemResponse { subscription }))
}
