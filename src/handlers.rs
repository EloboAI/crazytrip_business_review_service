use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

use crate::database::Database;
use crate::models::{
    ApiResponse, BusinessRegistration, BusinessRegistrationSummary,
    BusinessRegistrationWithHistory, BusinessVerificationStatus, CreateBusinessLocationRequest,
    CreateBusinessPromotionRequest, CreateBusinessRegistrationRequest, CreateCompanyRequest,
    CreateBusinessUnitRequest, ReviewAction, ReviewActionRequest, UpdateBusinessLocationRequest,
    UpdateBusinessPromotionRequest,
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

/// Health check endpoint
#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "business-review-service",
        "timestamp": chrono::Utc::now()
    }))
}

#[post("/registrations")]
pub async fn submit_registration(
    db: web::Data<Database>,
    payload: web::Json<CreateBusinessRegistrationRequest>,
) -> impl Responder {
    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Validation failed: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    let (new_registration, new_locations) = body.into_new_registration();
    match db
        .create_registration(new_registration, new_locations)
        .await
    {
        Ok((registration, locations)) => {
            HttpResponse::Created().json(ApiResponse::success(BusinessRegistrationWithHistory {
                registration,
                locations,
                promotions: Vec::new(),
                history: Vec::new(),
            }))
        }
        Err(err) => {
            log::error!("Failed to create registration: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to create registration".into(),
            ))
        }
    }
}

#[get("/registrations/{registration_id}")]
pub async fn get_registration(
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
) -> impl Responder {
    let registration_id = registration_id.into_inner();
    match db.get_registration_by_id(registration_id).await {
        Ok(Some(registration)) => {
            match build_registration_details(db.get_ref(), registration).await {
                Ok(details) => HttpResponse::Ok().json(ApiResponse::success(details)),
                Err(err) => {
                    log::error!("Failed to load registration details: {err:?}");
                    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        "Failed to load registration details".into(),
                    ))
                }
            }
        }
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to fetch registration".into(),
            ))
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
        Ok(Some(registration)) => {
            match build_registration_details(db.get_ref(), registration).await {
                Ok(details) => HttpResponse::Ok().json(ApiResponse::success(details)),
                Err(err) => {
                    log::error!("Failed to load registration details: {err:?}");
                    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        "Failed to load registration details".into(),
                    ))
                }
            }
        }
        Ok(None) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("No registrations for user".into())),
        Err(err) => {
            log::error!("Failed to fetch latest registration: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to fetch latest registration".into(),
            ))
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
            HttpResponse::InternalServerError().json(
                ApiResponse::<Vec<BusinessRegistrationSummary>>::error(
                    "Failed to list registrations".into(),
                ),
            )
        }
    }
}

#[post("/registrations/{registration_id}/locations")]
pub async fn create_location_for_registration(
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
    payload: web::Json<CreateBusinessLocationRequest>,
) -> impl Responder {
    let registration_id = registration_id.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to load registration".into(),
            ));
        }
    }

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Validation failed: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    let existing_locations = match db.list_locations_for_registration(registration_id).await {
        Ok(locations) => locations,
        Err(err) => {
            log::error!("Failed to list locations: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to load locations".into()));
        }
    };

    let new_location = body.into_new_location(registration_id, existing_locations.is_empty());

    match db.create_location_for_registration(new_location).await {
        Ok(_) => match db.get_registration_by_id(registration_id).await {
            Ok(Some(updated_registration)) => {
                match build_registration_details(db.get_ref(), updated_registration).await {
                    Ok(details) => HttpResponse::Created().json(ApiResponse::success(details)),
                    Err(err) => {
                        log::error!("Failed to load registration details: {err:?}");
                        HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                            "Failed to build registration response".into(),
                        ))
                    }
                }
            }
            Ok(None) => HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Registration not found".into())),
            Err(err) => {
                log::error!("Failed to reload registration: {err:?}");
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to reload registration".into(),
                ))
            }
        },
        Err(err) => {
            log::error!("Failed to create location: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to create location".into()))
        }
    }
}

#[put("/registrations/{registration_id}/locations/{location_id}")]
pub async fn update_location_for_registration(
    db: web::Data<Database>,
    path: web::Path<(Uuid, Uuid)>,
    payload: web::Json<UpdateBusinessLocationRequest>,
) -> impl Responder {
    let (registration_id, location_id) = path.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to load registration".into(),
            ));
        }
    }

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Validation failed: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    let existing_location = match db.get_location_by_id(registration_id, location_id).await {
        Ok(Some(location)) => location,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Location not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch location: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to load location".into()));
        }
    };

    let mut location = existing_location;
    body.apply_to_existing(&mut location);

    match db.update_location(location).await {
        Ok(_) => match db.get_registration_by_id(registration_id).await {
            Ok(Some(updated_registration)) => {
                match build_registration_details(db.get_ref(), updated_registration).await {
                    Ok(details) => HttpResponse::Ok().json(ApiResponse::success(details)),
                    Err(err) => {
                        log::error!("Failed to load registration details: {err:?}");
                        HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                            "Failed to build registration response".into(),
                        ))
                    }
                }
            }
            Ok(None) => HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Registration not found".into())),
            Err(err) => {
                log::error!("Failed to reload registration: {err:?}");
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to reload registration".into(),
                ))
            }
        },
        Err(err) => {
            log::error!("Failed to update location: {err:?}");
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to update location".into()))
        }
    }
}

#[delete("/registrations/{registration_id}/locations/{location_id}")]
pub async fn delete_location_for_registration(
    db: web::Data<Database>,
    path: web::Path<(Uuid, Uuid)>,
) -> impl Responder {
    let (registration_id, location_id) = path.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to load registration".into(),
            ));
        }
    }

    let locations = match db.list_locations_for_registration(registration_id).await {
        Ok(locations) => locations,
        Err(err) => {
            log::error!("Failed to list locations: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to load locations".into()));
        }
    };

    let target_location = match locations.iter().find(|loc| loc.id == location_id) {
        Some(location) => location,
        None => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Location not found".into()))
        }
    };

    if locations.len() == 1 {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "At least one location is required".into(),
        ));
    }

    let deleting_primary = target_location.is_primary;

    match db.delete_location(registration_id, location_id).await {
        Ok(_) => {}
        Err(sqlx::Error::RowNotFound) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Location not found".into()))
        }
        Err(err) => {
            log::error!("Failed to delete location: {err:?}");
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to delete location".into()));
        }
    }

    if deleting_primary {
        let remaining = match db.list_locations_for_registration(registration_id).await {
            Ok(locations) => locations,
            Err(err) => {
                log::error!("Failed to fetch remaining locations: {err:?}");
                return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Failed to load remaining locations".into(),
                ));
            }
        };

        let has_primary = remaining.iter().any(|loc| loc.is_primary);
        if !has_primary {
            if let Some(mut promote) = remaining.first().cloned() {
                promote.is_primary = true;
                if let Err(err) = db.update_location(promote).await {
                    log::error!(
                        "Failed to promote fallback primary location for registration {}: {err:?}",
                        registration_id
                    );
                    return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        "Failed to promote new primary location".into(),
                    ));
                }
            }
        }
    }

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(updated_registration)) => {
            match build_registration_details(db.get_ref(), updated_registration).await {
                Ok(details) => HttpResponse::Ok().json(ApiResponse::success(details)),
                Err(err) => {
                    log::error!("Failed to load registration details: {err:?}");
                    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        "Failed to build registration response".into(),
                    ))
                }
            }
        }
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to reload registration: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to reload registration".into(),
            ))
        }
    }
}

#[get("/registrations/{registration_id}/promotions")]
pub async fn list_promotions_for_registration(
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
) -> impl Responder {
    let registration_id = registration_id.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Registro de negocio no encontrado".into(),
            ))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo cargar el registro".into(),
            ));
        }
    }

    match db.list_promotions_for_registration(registration_id).await {
        Ok(promotions) => HttpResponse::Ok().json(ApiResponse::success(promotions)),
        Err(err) => {
            log::error!("Failed to list promotions: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudieron listar las promociones".into(),
            ))
        }
    }
}

#[get("/registrations/{registration_id}/promotions/{promotion_id}")]
pub async fn get_promotion_for_registration(
    db: web::Data<Database>,
    path: web::Path<(Uuid, Uuid)>,
) -> impl Responder {
    let (registration_id, promotion_id) = path.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Registro de negocio no encontrado".into(),
            ))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo cargar el registro".into(),
            ));
        }
    }

    match db
        .get_promotion_with_locations(registration_id, promotion_id)
        .await
    {
        Ok(Some(promotion)) => HttpResponse::Ok().json(ApiResponse::success(promotion)),
        Ok(None) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Promoción no encontrada".into())),
        Err(err) => {
            log::error!("Failed to load promotion: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la promoción".into(),
            ))
        }
    }
}

#[post("/registrations/{registration_id}/promotions")]
pub async fn create_promotion_for_registration(
    req: HttpRequest,
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
    payload: web::Json<CreateBusinessPromotionRequest>,
) -> impl Responder {
    let (actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let registration_id = registration_id.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Registro de negocio no encontrado".into(),
            ))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo cargar el registro".into(),
            ));
        }
    }

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Error de validación: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    if let Err(message) = body.validate_business_rules() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(message));
    }

    let (new_promotion, location_ids) = body.into_new_promotion(registration_id, Some(actor_id));

    match db.create_promotion(new_promotion, &location_ids).await {
        Ok(promotion) => HttpResponse::Created().json(ApiResponse::success(promotion)),
        Err(sqlx::Error::RowNotFound) => HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Una o más ubicaciones no pertenecen a esta solicitud".into(),
        )),
        Err(err) => {
            log::error!("Failed to create promotion: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo crear la promoción".into(),
            ))
        }
    }
}

#[put("/registrations/{registration_id}/promotions/{promotion_id}")]
pub async fn update_promotion_for_registration(
    req: HttpRequest,
    db: web::Data<Database>,
    path: web::Path<(Uuid, Uuid)>,
    payload: web::Json<UpdateBusinessPromotionRequest>,
) -> impl Responder {
    let (actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let (registration_id, promotion_id) = path.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Registro de negocio no encontrado".into(),
            ))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo cargar el registro".into(),
            ));
        }
    }

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Error de validación: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    if let Err(message) = body.validate_business_rules() {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(message));
    }

    let existing = match db
        .get_promotion_with_locations(registration_id, promotion_id)
        .await
    {
        Ok(Some(promotion)) => promotion,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Promoción no encontrada".into()))
        }
        Err(err) => {
            log::error!("Failed to load promotion: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la promoción".into(),
            ));
        }
    };

    let mut promotion = existing.promotion;
    let location_ids = body.apply_to_existing(&mut promotion, Some(actor_id));

    match db.update_promotion(promotion, &location_ids).await {
        Ok(updated) => HttpResponse::Ok().json(ApiResponse::success(updated)),
        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Promoción no encontrada".into())),
        Err(err) => {
            log::error!("Failed to update promotion: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo actualizar la promoción".into(),
            ))
        }
    }
}

#[delete("/registrations/{registration_id}/promotions/{promotion_id}")]
pub async fn delete_promotion_for_registration(
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

    let (registration_id, promotion_id) = path.into_inner();

    match db.get_registration_by_id(registration_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Registro de negocio no encontrado".into(),
            ))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo cargar el registro".into(),
            ));
        }
    }

    match db.delete_promotion(registration_id, promotion_id).await {
        Ok(()) => match db.list_promotions_for_registration(registration_id).await {
            Ok(promotions) => HttpResponse::Ok().json(ApiResponse::success(promotions)),
            Err(err) => {
                log::error!("Failed to list promotions after delete: {err:?}");
                HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "La promoción fue eliminada pero no se pudo listar el estado actual".into(),
                ))
            }
        },
        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Promoción no encontrada".into())),
        Err(err) => {
            log::error!("Failed to delete promotion: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo eliminar la promoción".into(),
            ))
        }
    }
}

// ==================== COMPANY & BUSINESS UNIT ENDPOINTS ====================

/// Create a new company for a user
#[post("/companies")]
pub async fn create_company(
    req: HttpRequest,
    db: web::Data<Database>,
    payload: web::Json<CreateCompanyRequest>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Error de validación: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    match db
        .create_company(
            body.owner_user_id,
            body.company_name,
            body.tax_id,
            body.legal_entity_type,
        )
        .await
    {
        Ok(company) => HttpResponse::Created().json(ApiResponse::success(company)),
        Err(err) => {
            log::error!("Failed to create company: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo crear la empresa".into(),
            ))
        }
    }
}

/// Get company details by ID
#[get("/companies/{company_id}")]
pub async fn get_company(
    db: web::Data<Database>,
    company_id: web::Path<Uuid>,
) -> impl Responder {
    let company_id = company_id.into_inner();
    match db.get_company(company_id).await {
        Ok(Some(company)) => HttpResponse::Ok().json(ApiResponse::success(company)),
        Ok(None) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Empresa no encontrada".into())),
        Err(err) => {
            log::error!("Failed to get company: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la empresa".into(),
            ))
        }
    }
}

/// List companies for the authenticated user
#[get("/companies")]
pub async fn list_companies(
    req: HttpRequest,
    db: web::Data<Database>,
) -> impl Responder {
    let (user_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    match db.list_companies_for_user(user_id).await {
        Ok(companies) => HttpResponse::Ok().json(ApiResponse::success(companies)),
        Err(err) => {
            log::error!("Failed to list companies: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo listar las empresas".into(),
            ))
        }
    }
}

/// Update company details
#[put("/companies/{company_id}")]
pub async fn update_company(
    req: HttpRequest,
    db: web::Data<Database>,
    company_id: web::Path<Uuid>,
    payload: web::Json<CreateCompanyRequest>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Error de validación: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    let company_id = company_id.into_inner();
    let mut company = match db.get_company(company_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Empresa no encontrada".into()));
        }
        Err(err) => {
            log::error!("Failed to fetch company: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la empresa".into(),
            ));
        }
    };

    company.company_name = body.company_name;
    company.tax_id = body.tax_id;
    company.legal_entity_type = body.legal_entity_type;

    match db.update_company(company).await {
        Ok(updated) => HttpResponse::Ok().json(ApiResponse::success(updated)),
        Err(err) => {
            log::error!("Failed to update company: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo actualizar la empresa".into(),
            ))
        }
    }
}

/// Delete a company
#[delete("/companies/{company_id}")]
pub async fn delete_company(
    req: HttpRequest,
    db: web::Data<Database>,
    company_id: web::Path<Uuid>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let company_id = company_id.into_inner();
    match db.delete_company(company_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Empresa no encontrada".into())),
        Err(err) => {
            log::error!("Failed to delete company: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo eliminar la empresa".into(),
            ))
        }
    }
}

/// Get company with all its business units
#[get("/companies/{company_id}/with-units")]
pub async fn get_company_with_units(
    db: web::Data<Database>,
    company_id: web::Path<Uuid>,
) -> impl Responder {
    let company_id = company_id.into_inner();
    match db.get_company_with_units(company_id).await {
        Ok(Some(company_with_units)) => {
            HttpResponse::Ok().json(ApiResponse::success(company_with_units))
        }
        Ok(None) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Empresa no encontrada".into())),
        Err(err) => {
            log::error!("Failed to get company with units: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la empresa con sus unidades".into(),
            ))
        }
    }
}

/// Create a business unit under a company
#[post("/companies/{company_id}/units")]
pub async fn create_business_unit(
    req: HttpRequest,
    db: web::Data<Database>,
    company_id: web::Path<Uuid>,
    payload: web::Json<CreateBusinessUnitRequest>,
) -> impl Responder {
    let (actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Error de validación: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    let company_id = company_id.into_inner();
    
    // Get or create a registration for this user to use as registrationId
    let registration_id = match db.get_or_create_auto_registration(actor_id, &body.unit_name, &body.category).await {
        Ok(reg_id) => Some(reg_id),
        Err(e) => {
            log::warn!("Could not get/create auto registration for user {actor_id}: {e:?}");
            None
        }
    };
    
    match db
        .create_business_unit(
            company_id,
            registration_id,
            body.unit_name,
            body.category,
            body.is_primary,
        )
        .await
    {
        Ok(unit) => HttpResponse::Created().json(ApiResponse::success(unit)),
        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound().json(ApiResponse::<()>::error(
            "Empresa no encontrada".into(),
        )),
        Err(err) => {
            log::error!("Failed to create business unit: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo crear la unidad de negocio".into(),
            ))
        }
    }
}

/// Get business unit details with locations
#[get("/units/{unit_id}")]
pub async fn get_business_unit(
    db: web::Data<Database>,
    unit_id: web::Path<Uuid>,
) -> impl Responder {
    let unit_id = unit_id.into_inner();
    match db.get_unit_detail(unit_id).await {
        Ok(Some(unit_detail)) => HttpResponse::Ok().json(ApiResponse::success(unit_detail)),
        Ok(None) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Unidad de negocio no encontrada".into())),
        Err(err) => {
            log::error!("Failed to get business unit: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la unidad de negocio".into(),
            ))
        }
    }
}

/// List business units for a company
#[get("/companies/{company_id}/units")]
pub async fn list_business_units(
    db: web::Data<Database>,
    company_id: web::Path<Uuid>,
) -> impl Responder {
    let company_id = company_id.into_inner();
    match db.list_units_for_company(company_id).await {
        Ok(units) => HttpResponse::Ok().json(ApiResponse::success(units)),
        Err(err) => {
            log::error!("Failed to list business units: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudieron listar las unidades de negocio".into(),
            ))
        }
    }
}

/// Update business unit
#[put("/units/{unit_id}")]
pub async fn update_business_unit(
    req: HttpRequest,
    db: web::Data<Database>,
    unit_id: web::Path<Uuid>,
    payload: web::Json<CreateBusinessUnitRequest>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let body = payload.into_inner();
    if let Err(e) = body.validate() {
        let error = format!("Error de validación: {}", e);
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(error));
    }

    let unit_id = unit_id.into_inner();
    let mut unit = match db.get_business_unit(unit_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Unidad de negocio no encontrada".into(),
            ));
        }
        Err(err) => {
            log::error!("Failed to fetch business unit: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la unidad de negocio".into(),
            ));
        }
    };

    unit.unit_name = body.unit_name;
    unit.category = body.category;
    unit.is_primary = body.is_primary;

    match db.update_business_unit(unit).await {
        Ok(updated) => HttpResponse::Ok().json(ApiResponse::success(updated)),
        Err(err) => {
            log::error!("Failed to update business unit: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo actualizar la unidad de negocio".into(),
            ))
        }
    }
}

/// Set a business unit as primary for its company
#[post("/units/{unit_id}/set-primary")]
pub async fn set_primary_unit(
    req: HttpRequest,
    db: web::Data<Database>,
    unit_id: web::Path<Uuid>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let unit_id = unit_id.into_inner();
    let unit = match db.get_business_unit(unit_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return HttpResponse::NotFound()
                .json(ApiResponse::<()>::error("Unidad de negocio no encontrada".into()));
        }
        Err(err) => {
            log::error!("Failed to fetch business unit: {err:?}");
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo obtener la unidad de negocio".into(),
            ));
        }
    };

    match db.set_primary_unit(unit.company_id, unit_id).await {
        Ok(()) => {
            // Fetch updated unit to return
            match db.get_business_unit(unit_id).await {
                Ok(Some(updated)) => HttpResponse::Ok().json(ApiResponse::success(updated)),
                Ok(None) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                    "Unidad actualizada pero no se pudo recuperar".into(),
                )),
                Err(err) => {
                    log::error!("Failed to fetch updated unit: {err:?}");
                    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        "Unidad actualizada pero no se pudo recuperar".into(),
                    ))
                }
            }
        }
        Err(err) => {
            log::error!("Failed to set primary unit: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo establecer la unidad como principal".into(),
            ))
        }
    }
}

/// Delete a business unit
#[delete("/units/{unit_id}")]
pub async fn delete_business_unit(
    req: HttpRequest,
    db: web::Data<Database>,
    unit_id: web::Path<Uuid>,
) -> impl Responder {
    let (_actor_id, _actor_name) = match extract_actor_headers(&req) {
        Ok(headers) => headers,
        Err(err) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(err));
        }
    };

    let unit_id = unit_id.into_inner();
    match db.delete_business_unit(unit_id).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound()
            .json(ApiResponse::<()>::error("Unidad de negocio no encontrada".into())),
        Err(err) => {
            log::error!("Failed to delete business unit: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "No se pudo eliminar la unidad de negocio".into(),
            ))
        }
    }
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// List pending businesses for review
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
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to list pending reviews".into(),
            ))
        }
    }
}

/// Get business review details including history
#[get("/reviews/{registration_id}")]
pub async fn get_business_review(
    db: web::Data<Database>,
    registration_id: web::Path<Uuid>,
) -> impl Responder {
    let registration_id = registration_id.into_inner();
    match db.get_registration_by_id(registration_id).await {
        Ok(Some(registration)) => {
            match build_registration_details(db.get_ref(), registration).await {
                Ok(details) => HttpResponse::Ok().json(ApiResponse::success(details)),
                Err(err) => {
                    log::error!("Failed to load registration details: {err:?}");
                    HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                        "Failed to load registration details".into(),
                    ))
                }
            }
        }
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("Registration not found".into()))
        }
        Err(err) => {
            log::error!("Failed to fetch registration: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to fetch registration".into(),
            ))
        }
    }
}

/// Submit review action (approve/reject/request_more_info)
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

    if reviewer_id.is_none()
        || reviewer_name
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "Reviewer identity is required".into(),
        ));
    }

    let new_status = match action {
        ReviewAction::Approve => BusinessVerificationStatus::Approved,
        ReviewAction::Reject => BusinessVerificationStatus::Rejected,
        ReviewAction::RequestMoreInfo => BusinessVerificationStatus::UnderReview,
        ReviewAction::Suspend => BusinessVerificationStatus::Suspended,
        ReviewAction::Resume => BusinessVerificationStatus::UnderReview,
        ReviewAction::Comment => existing.status.clone(),
    };

    match db
        .record_review_event(
            registration_id,
            reviewer_id,
            reviewer_name,
            action,
            notes,
            rejection_reason,
            new_status,
        )
        .await
    {
        Ok(updated_registration) => {
            match build_registration_details(db.get_ref(), updated_registration).await {
                Ok(details) => HttpResponse::Ok().json(ApiResponse::success(details)),
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
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to record review event".into(),
            ))
        }
    }
}

async fn build_registration_details(
    db: &Database,
    registration: BusinessRegistration,
) -> Result<BusinessRegistrationWithHistory, sqlx::Error> {
    let locations = db.list_locations_for_registration(registration.id).await?;
    let promotions = db.list_promotions_for_registration(registration.id).await?;
    let history = db.list_review_events(registration.id).await?;

    Ok(BusinessRegistrationWithHistory {
        registration,
        locations,
        promotions,
        history,
    })
}

/// Get review statistics for admin dashboard
#[get("/reviews/stats")]
pub async fn get_review_stats(db: web::Data<Database>) -> impl Responder {
    match db.get_review_stats().await {
        Ok(stats) => HttpResponse::Ok().json(ApiResponse::success(stats)),
        Err(err) => {
            log::error!("Failed to fetch review stats: {err:?}");
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to fetch review stats".into(),
            ))
        }
    }
}
