use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

// ============================================================================
// ENUMS
// ============================================================================

/// Business verification status (this is also a Postgres enum)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, postgres_types::ToSql, postgres_types::FromSql)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, postgres_types::ToSql, postgres_types::FromSql)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, postgres_types::ToSql, postgres_types::FromSql)]
#[sqlx(type_name = "business_promotion_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BusinessPromotionType {
    Discount,
    Contest,
    Event,
    Challenge,
}

/// Promotion lifecycle status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, postgres_types::ToSql, postgres_types::FromSql)]
#[sqlx(type_name = "business_promotion_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BusinessPromotionStatus {
    Draft,
    Scheduled,
    Active,
    Expired,
    Cancelled,
}

/// Location admin role
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, postgres_types::ToSql, postgres_types::FromSql)]
#[sqlx(type_name = "location_admin_role", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LocationAdminRole {
    Owner,
    Manager,
    Staff,
}

// ============================================================================
// BUSINESS REGISTRATION (Verification Workflow)
// ============================================================================

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

// ============================================================================
// APPROVED BUSINESSES
// ============================================================================

/// Approved business entity
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Business {
    pub id: Uuid,
    pub registration_id: Option<Uuid>,
    pub owner_user_id: Uuid,
    pub business_name: String,
    pub category: String,
    pub tax_id: Option<String>,
    pub description: Option<String>,
    pub website: Option<String>,
    pub logo_url: Option<String>,
    pub is_active: bool,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Helper for creating new business
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBusiness {
    pub id: Uuid,
    pub registration_id: Option<Uuid>,
    pub owner_user_id: Uuid,
    pub business_name: String,
    pub category: String,
    pub tax_id: Option<String>,
    pub description: Option<String>,
    pub website: Option<String>,
    pub logo_url: Option<String>,
    pub is_active: bool,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// BUSINESS LOCATIONS (Branches/Physical Locations)
// ============================================================================

/// Business location/branch
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessLocation {
    pub id: Uuid,
    pub business_id: Uuid,
    pub location_name: String,
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
    pub email: Option<String>,
    pub is_active: bool,
    pub is_primary: bool,
    pub operating_hours: Option<Value>,
    pub notes: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Helper for creating new location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBusinessLocation {
    pub id: Uuid,
    pub business_id: Uuid,
    pub location_name: String,
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
    pub email: Option<String>,
    pub is_active: bool,
    pub is_primary: bool,
    pub operating_hours: Option<Value>,
    pub notes: Option<String>,
    pub metadata: Value,
}

// ============================================================================
// BUSINESS PROMOTIONS (Per Location)
// ============================================================================

/// Promotion for a specific location
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessPromotion {
    pub id: Uuid,
    pub location_id: Uuid,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
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

/// Helper for creating new promotion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBusinessPromotion {
    pub id: Uuid,
    pub location_id: Uuid,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
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

// ============================================================================
// LOCATION ADMINISTRATORS
// ============================================================================

/// Administrator for a specific location
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LocationAdmin {
    pub id: Uuid,
    pub location_id: Uuid,
    pub user_id: Uuid,
    pub user_email: String,
    pub user_username: String,
    pub role: LocationAdminRole,
    pub granted_by: Option<Uuid>,
    pub granted_by_username: Option<String>,
    pub is_active: bool,
    pub granted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Helper for creating new admin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLocationAdmin {
    pub id: Uuid,
    pub location_id: Uuid,
    pub user_id: Uuid,
    pub user_email: String,
    pub user_username: String,
    pub role: LocationAdminRole,
    pub granted_by: Option<Uuid>,
    pub granted_by_username: Option<String>,
    pub is_active: bool,
    pub granted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// REQUEST/RESPONSE DTOs
// ============================================================================

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
}

impl CreateBusinessRegistrationRequest {
    pub fn into_new_registration(self) -> NewBusinessRegistration {
        let now = Utc::now();
        NewBusinessRegistration {
            id: Uuid::new_v4(),
            user_id: self.user_id,
            business_id: None,
            name: self.name,
            category: self.category,
            address: self.address,
            description: self.description,
            phone: self.phone,
            website: self.website,
            tax_id: self.tax_id,
            document_urls: self.document_urls,
            is_multi_user_team: self.is_multi_user_team,
            status: BusinessVerificationStatus::Pending,
            owner_email: self.owner_email,
            owner_username: self.owner_username,
            rejection_reason: None,
            reviewer_notes: None,
            reviewer_id: None,
            reviewer_name: None,
            submitted_at: now,
            updated_at: now,
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

/// Request to create a business
#[derive(Debug, Deserialize, Validate)]
pub struct CreateBusinessRequest {
    pub registration_id: Option<Uuid>,
    pub owner_user_id: Uuid,
    #[validate(length(min = 3, max = 120))]
    pub business_name: String,
    #[validate(length(min = 3, max = 120))]
    pub category: String,
    pub tax_id: Option<String>,
    pub description: Option<String>,
    pub website: Option<String>,
    pub logo_url: Option<String>,
}

impl CreateBusinessRequest {
    pub fn into_new_business(self) -> NewBusiness {
        let now = Utc::now();
        NewBusiness {
            id: Uuid::new_v4(),
            registration_id: self.registration_id,
            owner_user_id: self.owner_user_id,
            business_name: self.business_name,
            category: self.category,
            tax_id: self.tax_id,
            description: self.description,
            website: self.website,
            logo_url: self.logo_url,
            is_active: true,
            metadata: Value::Object(Default::default()),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Request to create a location
#[derive(Debug, Deserialize, Validate)]
pub struct CreateLocationRequest {
    #[validate(length(min = 2, max = 120))]
    pub location_name: String,
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
    pub email: Option<String>,
    pub is_primary: bool,
    pub operating_hours: Option<Value>,
    pub notes: Option<String>,
}

impl CreateLocationRequest {
    pub fn into_new_location(self, business_id: Uuid) -> NewBusinessLocation {
        NewBusinessLocation {
            id: Uuid::new_v4(),
            business_id,
            location_name: self.location_name,
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
            email: self.email,
            is_active: true,
            is_primary: self.is_primary,
            operating_hours: self.operating_hours,
            notes: self.notes,
            metadata: Value::Object(Default::default()),
        }
    }
}

/// Request to update a location
#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLocationRequest {
    #[validate(length(min = 2, max = 120))]
    pub location_name: String,
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
    pub email: Option<String>,
    pub is_primary: bool,
    pub is_active: bool,
    pub operating_hours: Option<Value>,
    pub notes: Option<String>,
}

impl UpdateLocationRequest {
    pub fn apply_to_existing(&self, existing: &mut BusinessLocation) {
        existing.location_name = self.location_name.clone();
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
        existing.email = self.email.clone();
        existing.is_primary = self.is_primary;
        existing.is_active = self.is_active;
        existing.operating_hours = self.operating_hours.clone();
        existing.notes = self.notes.clone();
        existing.updated_at = Utc::now();
    }
}

/// Request to create a promotion
#[derive(Debug, Deserialize, Validate)]
pub struct CreatePromotionRequest {
    #[validate(length(min = 3, max = 120))]
    pub title: String,
    #[validate(length(max = 160))]
    pub subtitle: Option<String>,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
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
    pub metadata: Option<Value>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
}

impl CreatePromotionRequest {
    pub fn validate_business_rules(&self) -> Result<(), String> {
        if self.ends_at <= self.starts_at {
            return Err("La fecha de finalización debe ser posterior a la fecha de inicio".into());
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
        location_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> NewBusinessPromotion {
        let now = Utc::now();
        let status = if self.starts_at > now {
            BusinessPromotionStatus::Scheduled
        } else {
            BusinessPromotionStatus::Active
        };

        NewBusinessPromotion {
            id: Uuid::new_v4(),
            location_id,
            title: self.title,
            subtitle: self.subtitle,
            description: self.description,
            promotion_type: self.promotion_type,
            status,
            image_url: self.image_url,
            prize: self.prize,
            reward_points: self.reward_points,
            discount_percent: self.discount_percent,
            max_claims: self.max_claims,
            per_user_limit: self.per_user_limit,
            total_claims: 0,
            requires_check_in: self.requires_check_in,
            requires_purchase: self.requires_purchase,
            terms: self.terms,
            metadata: self.metadata.unwrap_or(Value::Object(Default::default())),
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            published_at: None,
            created_by: actor_id,
            updated_by: actor_id,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Request to update a promotion
#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePromotionRequest {
    #[validate(length(min = 3, max = 120))]
    pub title: String,
    #[validate(length(max = 160))]
    pub subtitle: Option<String>,
    #[validate(length(max = 4000))]
    pub description: Option<String>,
    pub promotion_type: BusinessPromotionType,
    pub status: BusinessPromotionStatus,
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
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub metadata: Option<Value>,
}

impl UpdatePromotionRequest {
    pub fn validate_business_rules(&self) -> Result<(), String> {
        if self.ends_at <= self.starts_at {
            return Err("La fecha de finalización debe ser posterior a la fecha de inicio".into());
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

    pub fn apply_to_existing(&self, existing: &mut BusinessPromotion, actor_id: Option<Uuid>) {
        existing.title = self.title.clone();
        existing.subtitle = self.subtitle.clone();
        existing.description = self.description.clone();
        existing.promotion_type = self.promotion_type;
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
        existing.starts_at = self.starts_at;
        existing.ends_at = self.ends_at;
        existing.published_at = self.published_at;
        if let Some(metadata) = &self.metadata {
            existing.metadata = metadata.clone();
        }
        existing.updated_by = actor_id;
        existing.updated_at = Utc::now();
    }
}

/// Request to add location admin
#[derive(Debug, Deserialize, Validate)]
pub struct AddLocationAdminRequest {
    pub user_id: Uuid,
    #[validate(email)]
    pub user_email: String,
    #[validate(length(min = 3, max = 60))]
    pub user_username: String,
    pub role: LocationAdminRole,
}

impl AddLocationAdminRequest {
    pub fn into_new_admin(
        self,
        location_id: Uuid,
        granted_by: Option<Uuid>,
        granted_by_username: Option<String>,
    ) -> NewLocationAdmin {
        let now = Utc::now();
        NewLocationAdmin {
            id: Uuid::new_v4(),
            location_id,
            user_id: self.user_id,
            user_email: self.user_email,
            user_username: self.user_username,
            role: self.role,
            granted_by,
            granted_by_username,
            is_active: true,
            granted_at: now,
            created_at: now,
            updated_at: now,
        }
    }
}

// ============================================================================
// COMPOSITE RESPONSE TYPES
// ============================================================================

/// Business with its locations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessWithLocations {
    pub business: Business,
    pub locations: Vec<BusinessLocation>,
}

/// Location with its promotions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationWithPromotions {
    pub location: BusinessLocation,
    pub promotions: Vec<BusinessPromotion>,
}

/// Business registration with review history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationWithHistory {
    pub registration: BusinessRegistration,
    pub history: Vec<BusinessReviewEvent>,
}

/// Business registration summary (for list views with locations but without full history)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationSummary {
    #[serde(flatten)]
    pub registration: BusinessRegistration,
    pub locations: Vec<BusinessLocation>,
}

/// Location with admins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationWithAdmins {
    pub location: BusinessLocation,
    pub admins: Vec<LocationAdmin>,
}
