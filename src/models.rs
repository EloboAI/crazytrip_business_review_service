use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

/// Business verification status stored in Postgres enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "business_verification_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BusinessVerificationStatus {
    Pending,
    UnderReview,
    Approved,
    Rejected,
    Suspended,
}

/// Review actions applied by reviewers (also a Postgres enum)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "business_review_action", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    Approve,
    Reject,
    RequestMoreInfo,
    Suspend,
    Resume,
    Comment,
}

/// Promotion category type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "business_promotion_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BusinessPromotionType {
    Discount,
    Contest,
    Event,
    Challenge,
}

/// Scope selector for a promotion (business-wide vs specific location)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "business_promotion_scope", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BusinessPromotionScope {
    Business,
    Location,
}

/// Promotion lifecycle status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "business_promotion_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BusinessPromotionStatus {
    Draft,
    Scheduled,
    Active,
    Expired,
    Cancelled,
}

/// Business registration request persisted in database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessRegistration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub business_id: Option<Uuid>,
    pub name: String,
    pub category: String,
    pub address: String,
    pub description: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub tax_id: Option<String>,
    pub document_urls: Vec<String>,
    pub is_multi_user_team: bool,
    pub status: BusinessVerificationStatus,
    pub owner_email: String,
    pub owner_username: String,
    pub rejection_reason: Option<String>,
    pub reviewer_notes: Option<String>,
    pub reviewer_id: Option<Uuid>,
    pub reviewer_name: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Business company (owner-level entity)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessCompany {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub company_name: String,
    pub tax_id: Option<String>,
    pub legal_entity_type: Option<String>,
    pub is_active: bool,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Business unit (individual business under a company)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessUnit {
    pub id: Uuid,
    pub company_id: Uuid,
    pub registration_id: Option<Uuid>,
    pub business_id: Option<Uuid>,
    pub unit_name: String,
    pub category: String,
    pub is_primary: bool,
    pub is_active: bool,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Business location associated with a registration
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessLocation {
    pub id: Uuid,
    pub registration_id: Uuid,
    pub business_id: Option<Uuid>,
    pub label: String,
    pub formatted_address: String,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state_region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub google_place_id: Option<String>,
    pub timezone: Option<String>,
    pub phone: Option<String>,
    pub is_primary: bool,
    pub notes: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Promotion persisted in the database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessPromotion {
    pub id: Uuid,
    pub registration_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
    pub scope: BusinessPromotionScope,
    pub status: BusinessPromotionStatus,
    pub image_url: Option<String>,
    pub prize: Option<String>,
    pub reward_points: i32,
    pub discount_percent: Option<i32>,
    pub max_claims: Option<i32>,
    pub per_user_limit: Option<i32>,
    pub total_claims: i32,
    pub requires_check_in: bool,
    pub requires_purchase: bool,
    pub terms: Option<String>,
    pub metadata: Value,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Helper struct returned in API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessPromotionWithLocations {
    pub promotion: BusinessPromotion,
    pub locations: Vec<BusinessLocation>,
}

/// Helper for inserting a new promotion transactionally
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBusinessPromotion {
    pub id: Uuid,
    pub registration_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
    pub scope: BusinessPromotionScope,
    pub status: BusinessPromotionStatus,
    pub image_url: Option<String>,
    pub prize: Option<String>,
    pub reward_points: i32,
    pub discount_percent: Option<i32>,
    pub max_claims: Option<i32>,
    pub per_user_limit: Option<i32>,
    pub total_claims: i32,
    pub requires_check_in: bool,
    pub requires_purchase: bool,
    pub terms: Option<String>,
    pub metadata: Value,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Helper struct used when inserting a new location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBusinessLocation {
    pub id: Uuid,
    pub registration_id: Uuid,
    pub business_id: Option<Uuid>,
    pub label: String,
    pub formatted_address: String,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state_region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub google_place_id: Option<String>,
    pub timezone: Option<String>,
    pub phone: Option<String>,
    pub is_primary: bool,
    pub notes: Option<String>,
    pub metadata: Value,
}

/// Helper struct used when inserting a new registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBusinessRegistration {
    pub id: Uuid,
    pub user_id: Uuid,
    pub business_id: Option<Uuid>,
    pub name: String,
    pub category: String,
    pub address: String,
    pub description: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub tax_id: Option<String>,
    pub document_urls: Vec<String>,
    pub is_multi_user_team: bool,
    pub status: BusinessVerificationStatus,
    pub owner_email: String,
    pub owner_username: String,
    pub rejection_reason: Option<String>,
    pub reviewer_notes: Option<String>,
    pub reviewer_id: Option<Uuid>,
    pub reviewer_name: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Historical review event for auditing purposes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessReviewEvent {
    pub id: Uuid,
    pub registration_id: Uuid,
    pub reviewer_id: Option<Uuid>,
    pub reviewer_name: Option<String>,
    pub action: ReviewAction,
    pub notes: Option<String>,
    pub rejection_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Pending business registration for review dashboards
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PendingBusinessReview {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub address: String,
    pub tax_id: Option<String>,
    pub document_urls: Vec<String>,
    pub submitted_at: DateTime<Utc>,
    pub owner_email: String,
    pub owner_username: String,
}

/// Aggregated statistics for review dashboards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewStats {
    pub pending: i64,
    pub under_review: i64,
    pub approved_today: i64,
    pub rejected_today: i64,
}

/// Payload sent by business owners to create a registration
#[derive(Debug, Deserialize, Validate)]
pub struct CreateBusinessRegistrationRequest {
    pub user_id: Uuid,
    #[validate(length(min = 3, max = 120))]
    pub name: String,
    #[validate(length(min = 3, max = 120))]
    pub category: String,
    #[validate(length(min = 5))]
    pub address: String,
    #[validate(length(min = 10, max = 2000))]
    pub description: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    #[validate(length(min = 4, max = 64))]
    pub tax_id: Option<String>,
    #[validate(length(min = 1))]
    pub document_urls: Vec<String>,
    pub is_multi_user_team: bool,
    #[validate(email)]
    pub owner_email: String,
    #[validate(length(min = 3, max = 60))]
    pub owner_username: String,
    #[validate(length(min = 1))]
    #[validate(nested)]
    pub locations: Vec<CreateBusinessLocationRequest>,
}

/// Response wrapper that includes registration plus history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessRegistrationWithHistory {
    pub registration: BusinessRegistration,
    pub locations: Vec<BusinessLocation>,
    pub promotions: Vec<BusinessPromotionWithLocations>,
    pub history: Vec<BusinessReviewEvent>,
}

/// Company with nested units and their details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyWithUnits {
    pub company: BusinessCompany,
    pub units: Vec<BusinessUnitDetail>,
}

/// Business unit with related data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessUnitDetail {
    pub unit: BusinessUnit,
    pub registration: Option<BusinessRegistration>,
    pub locations: Vec<BusinessLocation>,
    pub promotions: Vec<BusinessPromotionWithLocations>,
}

/// Request to create a company
#[derive(Debug, Deserialize, Validate)]
pub struct CreateCompanyRequest {
    pub owner_user_id: Uuid,
    #[validate(length(min = 3, max = 120))]
    pub company_name: String,
    pub tax_id: Option<String>,
    pub legal_entity_type: Option<String>,
}

/// Request to create a business unit
#[derive(Debug, Deserialize, Validate)]
pub struct CreateBusinessUnitRequest {
    pub company_id: Uuid,
    #[validate(length(min = 3, max = 120))]
    pub unit_name: String,
    #[validate(length(min = 3, max = 120))]
    pub category: String,
    pub is_primary: bool,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

/// Review action request sent by reviewers
#[derive(Debug, Deserialize)]
pub struct ReviewActionRequest {
    pub action: ReviewAction,
    pub notes: Option<String>,
    pub rejection_reason: Option<String>,
    pub reviewer_id: Option<Uuid>,
    pub reviewer_name: Option<String>,
}

/// Request payload to create a new business location
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateBusinessLocationRequest {
    #[validate(length(min = 2, max = 120))]
    pub label: String,
    #[validate(length(min = 5))]
    pub formatted_address: String,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state_region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub google_place_id: Option<String>,
    pub timezone: Option<String>,
    pub phone: Option<String>,
    pub is_primary: bool,
    pub notes: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

/// Request payload to create a new promotion
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateBusinessPromotionRequest {
    #[validate(length(min = 3, max = 120))]
    pub title: String,
    #[validate(length(max = 160))]
    pub subtitle: Option<String>,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
    pub scope: BusinessPromotionScope,
    #[validate(length(max = 1024))]
    pub image_url: Option<String>,
    #[validate(length(max = 1024))]
    pub prize: Option<String>,
    #[validate(range(min = 0, max = 10000))]
    pub reward_points: i32,
    #[validate(range(min = 0, max = 100))]
    pub discount_percent: Option<i32>,
    #[validate(range(min = 1, max = 1000000))]
    pub max_claims: Option<i32>,
    #[validate(range(min = 1, max = 10000))]
    pub per_user_limit: Option<i32>,
    pub requires_check_in: bool,
    pub requires_purchase: bool,
    #[validate(length(max = 4000))]
    pub terms: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[serde(default)]
    pub location_ids: Vec<Uuid>,
}

/// Request payload to update an existing promotion
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateBusinessPromotionRequest {
    #[validate(length(min = 3, max = 120))]
    pub title: String,
    #[validate(length(max = 160))]
    pub subtitle: Option<String>,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
    pub scope: BusinessPromotionScope,
    #[validate(length(max = 1024))]
    pub image_url: Option<String>,
    #[validate(length(max = 1024))]
    pub prize: Option<String>,
    #[validate(range(min = 0, max = 10000))]
    pub reward_points: i32,
    #[validate(range(min = 0, max = 100))]
    pub discount_percent: Option<i32>,
    #[validate(range(min = 1, max = 1000000))]
    pub max_claims: Option<i32>,
    #[validate(range(min = 1, max = 10000))]
    pub per_user_limit: Option<i32>,
    pub status: BusinessPromotionStatus,
    pub requires_check_in: bool,
    pub requires_purchase: bool,
    #[validate(length(max = 4000))]
    pub terms: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[serde(default)]
    pub location_ids: Vec<Uuid>,
    pub published_at: Option<DateTime<Utc>>,
}

/// Request payload to update an existing business location
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UpdateBusinessLocationRequest {
    #[validate(length(min = 2, max = 120))]
    pub label: String,
    #[validate(length(min = 5))]
    pub formatted_address: String,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state_region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub google_place_id: Option<String>,
    pub timezone: Option<String>,
    pub phone: Option<String>,
    pub is_primary: bool,
    pub notes: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

/// Summary response for listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessRegistrationSummary {
    pub registration: BusinessRegistration,
    pub locations: Vec<BusinessLocation>,
}

impl CreateBusinessRegistrationRequest {
    pub fn into_new_registration(self) -> (NewBusinessRegistration, Vec<NewBusinessLocation>) {
        let now = Utc::now();
        let CreateBusinessRegistrationRequest {
            user_id,
            name,
            category,
            address,
            description,
            phone,
            website,
            tax_id,
            document_urls,
            is_multi_user_team,
            owner_email,
            owner_username,
            locations,
        } = self;

        let registration = NewBusinessRegistration {
            id: Uuid::new_v4(),
            user_id,
            business_id: None,
            name,
            category,
            address,
            description,
            phone,
            website,
            tax_id,
            document_urls,
            is_multi_user_team,
            status: BusinessVerificationStatus::Pending,
            owner_email,
            owner_username,
            rejection_reason: None,
            reviewer_notes: None,
            reviewer_id: None,
            reviewer_name: None,
            submitted_at: now,
            updated_at: now,
        };

        let registration_id = registration.id;

        let locations = locations
            .into_iter()
            .enumerate()
            .map(|(idx, location)| location.into_new_location(registration_id, idx == 0))
            .collect();

        (registration, locations)
    }
}

impl CreateBusinessLocationRequest {
    pub fn into_new_location(
        self,
        registration_id: Uuid,
        is_primary_default: bool,
    ) -> NewBusinessLocation {
        let metadata = sanitize_metadata(self.metadata);
        NewBusinessLocation {
            id: Uuid::new_v4(),
            registration_id,
            business_id: None,
            label: self.label,
            formatted_address: self.formatted_address,
            street: self.street,
            city: self.city,
            state_region: self.state_region,
            postal_code: self.postal_code,
            country: self.country,
            latitude: self.latitude,
            longitude: self.longitude,
            google_place_id: self.google_place_id,
            timezone: self.timezone,
            phone: self.phone,
            is_primary: self.is_primary || is_primary_default,
            notes: self.notes,
            metadata,
        }
    }
}

impl CreateBusinessPromotionRequest {
    pub fn validate_business_rules(&self) -> Result<(), String> {
        if self.ends_at <= self.starts_at {
            return Err("La fecha de finalización debe ser posterior a la fecha de inicio".into());
        }

        if self.scope == BusinessPromotionScope::Location && self.location_ids.is_empty() {
            return Err(
                "Debes seleccionar al menos una ubicación para promociones por sucursal".into(),
            );
        }

        if let Some(discount) = self.discount_percent {
            if self.promotion_type != BusinessPromotionType::Discount {
                return Err(
                    "El porcentaje de descuento solo aplica para promociones de tipo discount"
                        .into(),
                );
            }
            if !(0..=100).contains(&discount) {
                return Err("El descuento debe estar entre 0 y 100".into());
            }
        }

        if self.promotion_type == BusinessPromotionType::Contest && self.prize.is_none() {
            return Err("Las promociones de tipo concurso requieren especificar un premio".into());
        }

        Ok(())
    }

    pub fn into_new_promotion(
        self,
        registration_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> (NewBusinessPromotion, Vec<Uuid>) {
        let CreateBusinessPromotionRequest {
            title,
            subtitle,
            description,
            promotion_type,
            scope,
            image_url,
            prize,
            reward_points,
            discount_percent,
            max_claims,
            per_user_limit,
            requires_check_in,
            requires_purchase,
            terms,
            metadata,
            starts_at,
            ends_at,
            location_ids,
        } = self;

        let sanitized_metadata = sanitize_metadata(metadata);
        let now = Utc::now();
        let status = if starts_at > now {
            BusinessPromotionStatus::Scheduled
        } else {
            BusinessPromotionStatus::Active
        };

        let promotion = NewBusinessPromotion {
            id: Uuid::new_v4(),
            registration_id,
            unit_id: None,
            title,
            subtitle,
            description,
            promotion_type,
            scope,
            status,
            image_url,
            prize,
            reward_points,
            discount_percent,
            max_claims,
            per_user_limit,
            total_claims: 0,
            requires_check_in,
            requires_purchase,
            terms,
            metadata: sanitized_metadata,
            starts_at,
            ends_at,
            published_at: None,
            created_by: actor_id,
            updated_by: actor_id,
            created_at: now,
            updated_at: now,
        };

        (promotion, location_ids)
    }
}

impl UpdateBusinessPromotionRequest {
    pub fn validate_business_rules(&self) -> Result<(), String> {
        if self.ends_at <= self.starts_at {
            return Err("La fecha de finalización debe ser posterior a la fecha de inicio".into());
        }

        if self.scope == BusinessPromotionScope::Location && self.location_ids.is_empty() {
            return Err(
                "Debes seleccionar al menos una ubicación para promociones por sucursal".into(),
            );
        }

        if let Some(discount) = self.discount_percent {
            if self.promotion_type != BusinessPromotionType::Discount {
                return Err(
                    "El porcentaje de descuento solo aplica para promociones de tipo discount"
                        .into(),
                );
            }
            if !(0..=100).contains(&discount) {
                return Err("El descuento debe estar entre 0 y 100".into());
            }
        }

        if self.promotion_type == BusinessPromotionType::Contest && self.prize.is_none() {
            return Err("Las promociones de tipo concurso requieren especificar un premio".into());
        }

        Ok(())
    }

    pub fn apply_to_existing(
        &self,
        existing: &mut BusinessPromotion,
        actor_id: Option<Uuid>,
    ) -> Vec<Uuid> {
        existing.title = self.title.clone();
        existing.subtitle = self.subtitle.clone();
        existing.description = self.description.clone();
        existing.promotion_type = self.promotion_type;
        existing.scope = self.scope;
        existing.status = self.status;
        existing.image_url = self.image_url.clone();
        existing.prize = self.prize.clone();
        existing.reward_points = self.reward_points;
        existing.discount_percent = self.discount_percent;
        existing.max_claims = self.max_claims;
        existing.per_user_limit = self.per_user_limit;
        existing.requires_check_in = self.requires_check_in;
        existing.requires_purchase = self.requires_purchase;
        existing.terms = self.terms.clone();
        existing.metadata = sanitize_metadata(self.metadata.clone());
        existing.starts_at = self.starts_at;
        existing.ends_at = self.ends_at;
        existing.published_at = self.published_at;
        existing.updated_by = actor_id;
        existing.updated_at = Utc::now();

        self.location_ids.clone()
    }
}

impl UpdateBusinessLocationRequest {
    pub fn apply_to_existing(&self, existing: &mut BusinessLocation) {
        existing.label = self.label.clone();
        existing.formatted_address = self.formatted_address.clone();
        existing.street = self.street.clone();
        existing.city = self.city.clone();
        existing.state_region = self.state_region.clone();
        existing.postal_code = self.postal_code.clone();
        existing.country = self.country.clone();
        existing.latitude = self.latitude;
        existing.longitude = self.longitude;
        existing.google_place_id = self.google_place_id.clone();
        existing.timezone = self.timezone.clone();
        existing.phone = self.phone.clone();
        existing.is_primary = self.is_primary;
        existing.notes = self.notes.clone();
        existing.metadata = sanitize_metadata(self.metadata.clone());
    }
}

fn sanitize_metadata(metadata: Value) -> Value {
    match metadata {
        Value::Null => Value::Object(Default::default()),
        other => other,
    }
}
