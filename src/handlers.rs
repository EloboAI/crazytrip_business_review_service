use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

use crate::database::Database;
use crate::models::{
    AddLocationAdminRequest, ApiResponse, BusinessRegistration, CreateBusinessRequest,
    CreateBusinessRegistrationRequest, CreateLocationRequest, CreatePromotionRequest,
    ReviewAction, ReviewActionRequest, UpdateLocationRequest,
    UpdatePromotionRequest,
};

fn extract_actor_headers(req: &HttpRequest) -> Result<(Uuid, String), String> {
    let actor_id = req
        .headers()
        .get("X-Actor-Id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| "Missing or invalid X-Actor-Id header".to_string())?;

    let actor_name = req
        .headers()
        .get("X-Actor-Name")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing X-Actor-Name header".to_string())?;

    Ok((actor_id, actor_name))
}

// ============================================================================
// HEALTH CHECK
// ============================================================================

#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "business-review-service",
        "timestamp": chrono::Utc::now()
    }))
}

// ============================================================================
// BUSINESS REGISTRATIONS (Verification Workflow)
// ============================================================================

#[post("/registrations")]
pub async fn submit_registration(
    db: web::Data<Database>,
    payload: web::Json<CreateBusinessRegistrationRequest>,
) -> impl Responder {
    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    let new_registration = body.into_new_registration();
    match db.create_registration(new_registration).await {
        Ok(registration) => HttpResponse::Created().json(ApiResponse::success(registration)),
        Err(err) => {
            log::error!("Failed to create registration: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to create registration".into()))
        }
    }
}

#[get("/registrations/{registration_id}")]
pub async fn get_registration(
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
) -> impl Responder {
    let registration_id = registration_id.into_inner();
    match db.get_registration_with_history(registration_id).await {
        Ok(Some(details)) => HttpResponse::Ok().json(ApiResponse::success(details)),
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to fetch registration".into()))
        }
    }
}

#[get("/registrations/users/{user_id}/latest")]
pub async fn get_latest_registration_for_user(
    db: web::Data<Database>,
    user_id: web::Path<Uuid>,
) -> impl Responder {
    let user_id = user_id.into_inner();
    match db.get_latest_registration_for_user(user_id).await {
        Ok(Some(registration)) => HttpResponse::Ok().json(ApiResponse::success(registration)),
        Ok(None) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("No registrations for user".into())),
        Err(err) => {
            log::error!("Failed to fetch latest registration: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to fetch latest registration".into()))
        }
    }
}

#[get("/registrations/users/{user_id}")]
pub async fn list_registrations_for_user(
    db: web::Data<Database>,
    user_id: web::Path<Uuid>,
) -> impl Responder {
    let user_id = user_id.into_inner();
    match db.list_registrations_for_user(user_id).await {
        Ok(registrations) => HttpResponse::Ok().json(ApiResponse::success(registrations)),
        Err(err) => {
            log::error!("Failed to list registrations: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<Vec<BusinessRegistration>>::error(
                "Failed to list registrations".into(),
            ))
        }
    }
}

// ============================================================================
// REVIEW WORKFLOW
// ============================================================================

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[get("/reviews/pending")]
pub async fn list_pending_reviews(
    db: web::Data<Database>,
    query: web::Query<PaginationQuery>,
) -> impl Responder {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    match db.list_pending_reviews(limit, offset).await {
        Ok(records) => HttpResponse::Ok().json(ApiResponse::success(records)),
        Err(err) => {
            log::error!("Failed to list pending reviews: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to list pending reviews".into()))
        }
    }
}

#[get("/reviews/{registration_id}")]
pub async fn get_business_review(
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
) -> impl Responder {
    let registration_id = registration_id.into_inner();
    match db.get_registration_with_history(registration_id).await {
        Ok(Some(details)) => HttpResponse::Ok().json(ApiResponse::success(details)),
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to fetch registration".into()))
        }
    }
}

#[post("/reviews/{registration_id}/action")]
pub async fn submit_review_action(
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
    payload: web::Json<ReviewActionRequest>,
) -> impl Responder {
    let registration_id = registration_id.into_inner();

    let existing = match db.get_registration_by_id(registration_id).await {
        Ok(Some(reg)) => reg,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to process review".into()));
        }
    };

    let payload = payload.into_inner();
    let ReviewActionRequest {
        action,
        notes,
        rejection_reason,
        reviewer_id,
        reviewer_name,
    } = payload;

    if matches!(action, ReviewAction::Reject) && rejection_reason.is_none() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Rejection reason is required when rejecting a registration".into(),
        ));
    }

    // Use default reviewer name if not provided
    let final_reviewer_name = reviewer_name
        .filter(|s| !s.trim().is_empty())
        .or_else(|| Some("Admin".to_string()));

    let new_status = match action {
        ReviewAction::Approve => crate::models::BusinessVerificationStatus::Approved,
        ReviewAction::Reject => crate::models::BusinessVerificationStatus::Rejected,
        ReviewAction::RequestMoreInfo => crate::models::BusinessVerificationStatus::UnderReview,
        ReviewAction::Suspend => crate::models::BusinessVerificationStatus::Suspended,
        ReviewAction::Resume => crate::models::BusinessVerificationStatus::UnderReview,
        ReviewAction::Comment => existing.status,
    };

    match db
        .record_review_event(
            registration_id,
            reviewer_id,
            final_reviewer_name,
            action,
            notes,
            rejection_reason,
            new_status,
        )
        .await
    {
        Ok(_updated_registration) => {
            match db.get_registration_with_history(registration_id).await {
                Ok(Some(details)) => HttpResponse::Ok().json(ApiResponse::success(details)),
                Ok(None) => HttpResponse::NotFound()
                    .json(ApiResponse::<()>::error("Registration not found".into())),
                Err(err) => {
                    log::error!("Failed to load registration details: {err:?}");
                    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        "Failed to load registration details".into(),
                    ))
                }
            }
        }
        Err(err) => {
            log::error!("Failed to record review event: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to record review event".into()))
        }
    }
}

#[get("/reviews/stats")]
pub async fn get_review_stats(db: web::Data<Database>) -> impl Responder {
    match db.get_review_stats().await {
        Ok(stats) => HttpResponse::Ok().json(ApiResponse::success(stats)),
        Err(err) => {
            log::error!("Failed to fetch review stats: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to fetch review stats".into()))
        }
    }
}

// ============================================================================
// BUSINESSES
// ============================================================================

#[post("/businesses")]
pub async fn create_business(
    req: HttpRequest,
    db: web::Data<Database>,
    payload: web::Json<CreateBusinessRequest>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    let new_business = body.into_new_business();
    match db.create_business(new_business).await {
        Ok(business) => HttpResponse::Created().json(ApiResponse::success(business)),
        Err(err) => {
            log::error!("Failed to create business: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to create business".into()))
        }
    }
}

#[get("/businesses/{business_id}")]
pub async fn get_business(db: web::Data<Database>, business_id: web::Path<Uuid>) -> impl Responder {
    let business_id = business_id.into_inner();
    match db.get_business(business_id).await {
        Ok(Some(business)) => HttpResponse::Ok().json(ApiResponse::success(business)),
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Business not found".into()))
        }
        Err(err) => {
            log::error!("Failed to get business: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to get business".into()))
        }
    }
}

#[get("/businesses/users/{user_id}")]
pub async fn list_businesses_for_user(
    db: web::Data<Database>,
    user_id: web::Path<Uuid>,
) -> impl Responder {
    let user_id = user_id.into_inner();
    match db.list_businesses_for_user(user_id).await {
        Ok(businesses) => HttpResponse::Ok().json(ApiResponse::success(businesses)),
        Err(err) => {
            log::error!("Failed to list businesses: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to list businesses".into()))
        }
    }
}

#[put("/businesses/{business_id}")]
pub async fn update_business(
    req: HttpRequest,
    db: web::Data<Database>,
    business_id: web::Path<Uuid>,
    payload: web::Json<CreateBusinessRequest>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let business_id = business_id.into_inner();
    let body = payload.into_inner();

    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    let mut existing_business = match db.get_business(business_id).await {
        Ok(Some(biz)) => biz,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Business not found".into()));
        }
        Err(err) => {
            log::error!("Failed to fetch business: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to load business".into()));
        }
    };

    existing_business.business_name = body.business_name;
    existing_business.tax_id = body.tax_id;
    existing_business.category = body.category;
    existing_business.description = body.description;
    existing_business.website = body.website;
    existing_business.logo_url = body.logo_url;

    match db.update_business(existing_business).await {
        Ok(updated) => HttpResponse::Ok().json(ApiResponse::success(updated)),
        Err(err) => {
            log::error!("Failed to update business: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to update business".into()))
        }
    }
}

#[delete("/businesses/{business_id}")]
pub async fn delete_business(
    req: HttpRequest,
    db: web::Data<Database>,
    business_id: web::Path<Uuid>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let business_id = business_id.into_inner();
    match db.delete_business(business_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(sqlx::Error::RowNotFound) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Business not found".into()))
        }
        Err(err) => {
            log::error!("Failed to delete business: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to delete business".into()))
        }
    }
}

// ============================================================================
// BUSINESS LOCATIONS
// ============================================================================

#[post("/businesses/{business_id}/locations")]
pub async fn create_location(
    req: HttpRequest,
    db: web::Data<Database>,
    business_id: web::Path<Uuid>,
    payload: web::Json<CreateLocationRequest>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let business_id = business_id.into_inner();
    let body = payload.into_inner();

    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    let new_location = body.into_new_location(business_id);
    match db.create_location(new_location).await {
        Ok(location) => HttpResponse::Created().json(ApiResponse::success(location)),
        Err(err) => {
            log::error!("Failed to create location: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to create location".into()))
        }
    }
}

#[get("/locations/{location_id}")]
pub async fn get_location(db: web::Data<Database>, location_id: web::Path<Uuid>) -> impl Responder {
    let location_id = location_id.into_inner();
    match db.get_location(location_id).await {
        Ok(Some(location)) => HttpResponse::Ok().json(ApiResponse::success(location)),
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Location not found".into()))
        }
        Err(err) => {
            log::error!("Failed to get location: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to get location".into()))
        }
    }
}

#[get("/businesses/{business_id}/locations")]
pub async fn list_locations_for_business(
    db: web::Data<Database>,
    business_id: web::Path<Uuid>,
) -> impl Responder {
    let business_id = business_id.into_inner();
    match db.list_locations_for_business(business_id).await {
        Ok(locations) => HttpResponse::Ok().json(ApiResponse::success(locations)),
        Err(err) => {
            log::error!("Failed to list locations: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to list locations".into()))
        }
    }
}

#[put("/locations/{location_id}")]
pub async fn update_location(
    req: HttpRequest,
    db: web::Data<Database>,
    location_id: web::Path<Uuid>,
    payload: web::Json<UpdateLocationRequest>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let location_id = location_id.into_inner();
    let body = payload.into_inner();

    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    let mut existing_location = match db.get_location(location_id).await {
        Ok(Some(loc)) => loc,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Location not found".into()));
        }
        Err(err) => {
            log::error!("Failed to fetch location: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to load location".into()));
        }
    };

    body.apply_to_existing(&mut existing_location);

    match db.update_location(existing_location).await {
        Ok(updated) => HttpResponse::Ok().json(ApiResponse::success(updated)),
        Err(err) => {
            log::error!("Failed to update location: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to update location".into()))
        }
    }
}

#[delete("/locations/{location_id}")]
pub async fn delete_location(
    req: HttpRequest,
    db: web::Data<Database>,
    location_id: web::Path<Uuid>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let location_id = location_id.into_inner();
    match db.delete_location(location_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(sqlx::Error::RowNotFound) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Location not found".into()))
        }
        Err(err) => {
            log::error!("Failed to delete location: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to delete location".into()))
        }
    }
}

// ============================================================================
// BUSINESS PROMOTIONS
// ============================================================================

#[post("/locations/{location_id}/promotions")]
pub async fn create_promotion(
    req: HttpRequest,
    db: web::Data<Database>,
    location_id: web::Path<Uuid>,
    payload: web::Json<CreatePromotionRequest>,
) -> impl Responder {
    let (actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let location_id = location_id.into_inner();
    let body = payload.into_inner();

    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    if let Err(message) = body.validate_business_rules() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(message));
    }

    let new_promotion = body.into_new_promotion(location_id, Some(actor_id));
    match db.create_promotion(new_promotion).await {
        Ok(promotion) => HttpResponse::Created().json(ApiResponse::success(promotion)),
        Err(err) => {
            log::error!("Failed to create promotion: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to create promotion".into()))
        }
    }
}

#[get("/promotions/{promotion_id}")]
pub async fn get_promotion(
    db: web::Data<Database>,
    promotion_id: web::Path<Uuid>,
) -> impl Responder {
    let promotion_id = promotion_id.into_inner();
    match db.get_promotion(promotion_id).await {
        Ok(Some(promotion)) => HttpResponse::Ok().json(ApiResponse::success(promotion)),
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Promotion not found".into()))
        }
        Err(err) => {
            log::error!("Failed to get promotion: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to get promotion".into()))
        }
    }
}

#[get("/locations/{location_id}/promotions")]
pub async fn list_promotions_for_location(
    db: web::Data<Database>,
    location_id: web::Path<Uuid>,
) -> impl Responder {
    let location_id = location_id.into_inner();
    match db.list_promotions_for_location(location_id).await {
        Ok(promotions) => HttpResponse::Ok().json(ApiResponse::success(promotions)),
        Err(err) => {
            log::error!("Failed to list promotions: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to list promotions".into()))
        }
    }
}

#[get("/businesses/{business_id}/promotions")]
pub async fn list_promotions_for_business(
    db: web::Data<Database>,
    business_id: web::Path<Uuid>,
) -> impl Responder {
    let business_id = business_id.into_inner();
    match db.list_promotions_for_business(business_id).await {
        Ok(promotions) => HttpResponse::Ok().json(ApiResponse::success(promotions)),
        Err(err) => {
            log::error!("Failed to list promotions: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to list promotions".into()))
        }
    }
}

#[put("/promotions/{promotion_id}")]
pub async fn update_promotion(
    req: HttpRequest,
    db: web::Data<Database>,
    promotion_id: web::Path<Uuid>,
    payload: web::Json<UpdatePromotionRequest>,
) -> impl Responder {
    let (actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let promotion_id = promotion_id.into_inner();
    let body = payload.into_inner();

    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    if let Err(message) = body.validate_business_rules() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(message));
    }

    let mut existing_promotion = match db.get_promotion(promotion_id).await {
        Ok(Some(promo)) => promo,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Promotion not found".into()));
        }
        Err(err) => {
            log::error!("Failed to fetch promotion: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to load promotion".into()));
        }
    };

    body.apply_to_existing(&mut existing_promotion, Some(actor_id));

    match db.update_promotion(existing_promotion).await {
        Ok(updated) => HttpResponse::Ok().json(ApiResponse::success(updated)),
        Err(err) => {
            log::error!("Failed to update promotion: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to update promotion".into()))
        }
    }
}

#[delete("/promotions/{promotion_id}")]
pub async fn delete_promotion(
    req: HttpRequest,
    db: web::Data<Database>,
    promotion_id: web::Path<Uuid>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let promotion_id = promotion_id.into_inner();
    match db.delete_promotion(promotion_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(sqlx::Error::RowNotFound) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Promotion not found".into()))
        }
        Err(err) => {
            log::error!("Failed to delete promotion: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to delete promotion".into()))
        }
    }
}

// ============================================================================
// LOCATION ADMINISTRATORS
// ============================================================================

#[post("/locations/{location_id}/admins")]
pub async fn add_location_admin(
    req: HttpRequest,
    db: web::Data<Database>,
    location_id: web::Path<Uuid>,
    payload: web::Json<AddLocationAdminRequest>,
) -> impl Responder {
    let (actor_id, actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let location_id = location_id.into_inner();
    let body = payload.into_inner();

    if let Err(e) = body.validate() {
        return HttpResponse::BadRequest()
            .json(ApiResponse::<()>::error(format!("Validation failed: {}", e)));
    }

    let new_admin = body.into_new_admin(location_id, Some(actor_id), Some(actor_name));
    match db.add_location_admin(new_admin).await {
        Ok(admin) => HttpResponse::Created().json(ApiResponse::success(admin)),
        Err(err) => {
            log::error!("Failed to add location admin: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to add location admin".into()))
        }
    }
}

#[get("/locations/{location_id}/admins")]
pub async fn list_location_admins(
    db: web::Data<Database>,
    location_id: web::Path<Uuid>,
) -> impl Responder {
    let location_id = location_id.into_inner();
    match db.list_location_admins(location_id).await {
        Ok(admins) => HttpResponse::Ok().json(ApiResponse::success(admins)),
        Err(err) => {
            log::error!("Failed to list location admins: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to list location admins".into()))
        }
    }
}

#[delete("/locations/{location_id}/admins/{user_id}")]
pub async fn remove_location_admin(
    req: HttpRequest,
    db: web::Data<Database>,
    path: web::Path<(Uuid, Uuid)>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let (location_id, user_id) = path.into_inner();
    match db.remove_location_admin(location_id, user_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(err) => {
            log::error!("Failed to remove location admin: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to remove location admin".into()))
        }
    }
}
