use chrono::{DateTime, Utc};
use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;
use uuid::Uuid;

use crate::models::{
    Business, BusinessLocation, BusinessPromotion, BusinessRegistration, BusinessReviewEvent,
    BusinessVerificationStatus, LocationAdmin, NewBusiness, NewBusinessLocation,
    NewBusinessPromotion, NewBusinessRegistration, NewLocationAdmin, PendingBusinessReview,
    RegistrationWithHistory, ReviewAction, ReviewStats,
};

type Error = Box<dyn std::error::Error + Send + Sync>;

pub type DbPool = Pool;

pub struct DatabaseService {
    pool: DbPool,
}

impl DatabaseService {
    pub async fn new(database_url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut cfg = Config::new();
        cfg.url = Some(database_url.to_string());
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
        let client = pool.get().await?;
        client.execute("SELECT 1", &[]).await?;

        log::info!("Database connection established");
        Ok(Self { pool })
    }

    pub async fn get_client(
        &self,
    ) -> Result<deadpool_postgres::Client, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.pool.get().await?)
    }

    /// Initialize database schema
    pub async fn init_schema(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Schema creation is intentionally omitted here.
        // Use the SQL files under `migrations/` and the `bin/` helpers to create or migrate the database.
        log::info!("Skipping inline DDL in init_schema; use migrations/ and bin/ scripts to manage schema");
        Ok(())
    }

    // ========================================================================
    // BUSINESS REGISTRATION (Verification Workflow)
    // ========================================================================

    pub async fn create_registration(
        &self,
        registration: NewBusinessRegistration,
    ) -> Result<BusinessRegistration, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client().await?;

        let row = client.query_one(
            r#"
            INSERT INTO business_registration_requests (
                id, user_id, business_id, name, category, address, description,
                phone, website, tax_id, document_urls, is_multi_user_team, status,
                owner_email, owner_username, rejection_reason, reviewer_notes,
                reviewer_id, reviewer_name, submitted_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
            RETURNING *
            "#,
            &[
                &registration.id,
                &registration.user_id,
                &registration.business_id,
                &registration.name,
                &registration.category,
                &registration.address,
                &registration.description,
                &registration.phone,
                &registration.website,
                &registration.tax_id,
                &registration.document_urls,
                &registration.is_multi_user_team,
                &registration.status,
                &registration.owner_email,
                &registration.owner_username,
                &registration.rejection_reason,
                &registration.reviewer_notes,
                &registration.reviewer_id,
                &registration.reviewer_name,
                &registration.submitted_at,
                &registration.updated_at,
            ],
        ).await?;

        Ok(row_to_business_registration(&row))
    }

    pub async fn get_registration_by_id(
        &self,
        registration_id: Uuid,
    ) -> Result<Option<BusinessRegistration>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client().await?;

        let rows = client.query(
            "SELECT * FROM business_registration_requests WHERE id = $1",
            &[&registration_id],
        ).await?;

        if rows.is_empty() {
            Ok(None)
        } else {
            Ok(Some(row_to_business_registration(&rows[0])))
        }
    }

    pub async fn get_latest_registration_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<BusinessRegistration>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client().await?;

        let rows = client.query(
            r#"
            SELECT * FROM business_registration_requests
            WHERE user_id = $1
            ORDER BY submitted_at DESC
            LIMIT 1
            "#,
            &[&user_id],
        ).await?;

        if rows.is_empty() {
            Ok(None)
        } else {
            Ok(Some(row_to_business_registration(&rows[0])))
        }
    }

    pub async fn list_registrations_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<BusinessRegistration>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client().await?;

        let rows = client.query(
            r#"
            SELECT * FROM business_registration_requests
            WHERE user_id = $1
            ORDER BY submitted_at DESC
            "#,
            &[&user_id],
        ).await?;

        Ok(rows.iter().map(row_to_business_registration).collect())
    }

    pub async fn list_pending_reviews(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PendingBusinessReview>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client().await?;

        let rows = client.query(
            r#"
            SELECT id, name, category, address, tax_id, document_urls,
                   submitted_at, owner_email, owner_username
            FROM business_registration_requests
            WHERE status = 'pending'
            ORDER BY submitted_at ASC
            LIMIT $1 OFFSET $2
            "#,
            &[&limit, &offset],
        ).await?;

        Ok(rows.iter().map(row_to_pending_business_review).collect())
    }

    // TODO: Port remaining methods (record_review_event, list_review_events, get_registration_with_history, get_review_stats, etc.)
    pub async fn record_review_event(
        &self,
        registration_id: Uuid,
        reviewer_id: Option<Uuid>,
        reviewer_name: Option<String>,
        action: ReviewAction,
        notes: Option<String>,
        rejection_reason: Option<String>,
        new_status: BusinessVerificationStatus,
    ) -> Result<BusinessRegistration, Error> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;

        tx.execute(
            "INSERT INTO business_review_events (registration_id, reviewer_id, reviewer_name, action, notes, rejection_reason) VALUES ($1, $2, $3, $4, $5, $6)",
            &[&registration_id, &reviewer_id, &reviewer_name, &action, &notes, &rejection_reason],
        ).await?;

        let row = tx.query_one(
            "UPDATE business_registration_requests SET status = $2, rejection_reason = $3, reviewer_notes = COALESCE($4, reviewer_notes), reviewer_id = COALESCE($5, reviewer_id), reviewer_name = COALESCE($6, reviewer_name), updated_at = NOW() WHERE id = $1 RETURNING id, user_id, business_id, name, category, address, description, phone, website, tax_id, document_urls, is_multi_user_team, status, owner_email, owner_username, rejection_reason, reviewer_notes, reviewer_id, reviewer_name, submitted_at, updated_at",
            &[&registration_id, &new_status, &rejection_reason, &notes, &reviewer_id, &reviewer_name],
        ).await?;

        tx.commit().await?;

        Ok(row_to_business_registration(&row))
    }

    pub async fn create_business(&self, business: NewBusiness) -> Result<Business, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "INSERT INTO businesses (id, registration_id, owner_user_id, business_name, category, tax_id, description, website, logo_url, is_active, metadata, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) RETURNING id, registration_id, owner_user_id, business_name, category, tax_id, description, website, logo_url, is_active, metadata, created_at, updated_at",
            &[&business.id, &business.registration_id, &business.owner_user_id, &business.business_name, &business.category, &business.tax_id, &business.description, &business.website, &business.logo_url, &business.is_active, &business.metadata, &business.created_at, &business.updated_at],
        ).await?;

        Ok(row_to_business(&row))
    }

    pub async fn get_business(&self, business_id: Uuid) -> Result<Option<Business>, Error> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT id, registration_id, owner_user_id, business_name, category, tax_id, description, website, logo_url, is_active, metadata, created_at, updated_at FROM businesses WHERE id = $1",
            &[&business_id],
        ).await?;

        Ok(row.map(|r| row_to_business(&r)))
    }

    pub async fn list_businesses_for_user(&self, user_id: Uuid) -> Result<Vec<Business>, Error> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT id, registration_id, owner_user_id, business_name, category, tax_id, description, website, logo_url, is_active, metadata, created_at, updated_at FROM businesses WHERE owner_user_id = $1 ORDER BY created_at DESC",
            &[&user_id],
        ).await?;

        Ok(rows.into_iter().map(|r| row_to_business(&r)).collect())
    }

    pub async fn update_business(&self, business: Business) -> Result<Business, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "UPDATE businesses SET registration_id = $2, business_name = $3, category = $4, tax_id = $5, description = $6, website = $7, logo_url = $8, is_active = $9, metadata = $10, updated_at = NOW() WHERE id = $1 RETURNING id, registration_id, owner_user_id, business_name, category, tax_id, description, website, logo_url, is_active, metadata, created_at, updated_at",
            &[&business.id, &business.registration_id, &business.owner_user_id, &business.business_name, &business.category, &business.tax_id, &business.description, &business.website, &business.logo_url, &business.is_active, &business.metadata],
        ).await?;

        Ok(row_to_business(&row))
    }

    pub async fn delete_business(&self, business_id: Uuid) -> Result<(), Error> {
        let client = self.pool.get().await?;
        client.execute("DELETE FROM businesses WHERE id = $1", &[&business_id]).await?;
        Ok(())
    }

    pub async fn create_location(&self, location: NewBusinessLocation) -> Result<BusinessLocation, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "INSERT INTO business_locations (id, business_id, location_name, formatted_address, street, city, state_region, postal_code, country, latitude, longitude, google_place_id, timezone, phone, email, is_active, is_primary, operating_hours, notes, metadata) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20) RETURNING id, business_id, location_name, formatted_address, street, city, state_region, postal_code, country, latitude, longitude, google_place_id, timezone, phone, email, is_active, is_primary, operating_hours, notes, metadata, created_at, updated_at",
            &[&location.id, &location.business_id, &location.location_name, &location.formatted_address, &location.street, &location.city, &location.state_region, &location.postal_code, &location.country, &location.latitude, &location.longitude, &location.google_place_id, &location.timezone, &location.phone, &location.email, &location.is_active, &location.is_primary, &location.operating_hours, &location.notes, &location.metadata],
        ).await?;

        Ok(row_to_business_location(&row))
    }

    pub async fn get_location(&self, location_id: Uuid) -> Result<Option<BusinessLocation>, Error> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT id, business_id, location_name, formatted_address, street, city, state_region, postal_code, country, latitude, longitude, google_place_id, timezone, phone, email, is_active, is_primary, operating_hours, notes, metadata, created_at, updated_at FROM business_locations WHERE id = $1",
            &[&location_id],
        ).await?;

        Ok(row.map(|r| row_to_business_location(&r)))
    }

    pub async fn list_locations_for_business(&self, business_id: Uuid) -> Result<Vec<BusinessLocation>, Error> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT id, business_id, location_name, formatted_address, street, city, state_region, postal_code, country, latitude, longitude, google_place_id, timezone, phone, email, is_active, is_primary, operating_hours, notes, metadata, created_at, updated_at FROM business_locations WHERE business_id = $1 ORDER BY is_primary DESC, created_at ASC",
            &[&business_id],
        ).await?;

        Ok(rows.into_iter().map(|r| row_to_business_location(&r)).collect())
    }

    pub async fn update_location(&self, location: BusinessLocation) -> Result<BusinessLocation, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "UPDATE business_locations SET location_name = $2, formatted_address = $3, street = $4, city = $5, state_region = $6, postal_code = $7, country = $8, latitude = $9, longitude = $10, google_place_id = $11, timezone = $12, phone = $13, email = $14, is_active = $15, is_primary = $16, operating_hours = $17, notes = $18, metadata = $19, updated_at = NOW() WHERE id = $1 RETURNING id, business_id, location_name, formatted_address, street, city, state_region, postal_code, country, latitude, longitude, google_place_id, timezone, phone, email, is_active, is_primary, operating_hours, notes, metadata, created_at, updated_at",
            &[&location.id, &location.location_name, &location.formatted_address, &location.street, &location.city, &location.state_region, &location.postal_code, &location.country, &location.latitude, &location.longitude, &location.google_place_id, &location.timezone, &location.phone, &location.email, &location.is_active, &location.is_primary, &location.operating_hours, &location.notes, &location.metadata],
        ).await?;

        Ok(row_to_business_location(&row))
    }

    pub async fn delete_location(&self, location_id: Uuid) -> Result<(), Error> {
        let client = self.pool.get().await?;
        client.execute("DELETE FROM business_locations WHERE id = $1", &[&location_id]).await?;
        Ok(())
    }

    pub async fn create_promotion(&self, promotion: NewBusinessPromotion) -> Result<BusinessPromotion, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "INSERT INTO business_promotions (id, location_id, title, subtitle, description, promotion_type, status, image_url, prize, reward_points, discount_percent, max_claims, per_user_limit, total_claims, requires_check_in, requires_purchase, terms, metadata, starts_at, ends_at, published_at, created_by, updated_by, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25) RETURNING id, location_id, title, subtitle, description, promotion_type, status, image_url, prize, reward_points, discount_percent, max_claims, per_user_limit, total_claims, requires_check_in, requires_purchase, terms, metadata, starts_at, ends_at, published_at, created_by, updated_by, created_at, updated_at",
            &[&promotion.id, &promotion.location_id, &promotion.title, &promotion.subtitle, &promotion.description, &promotion.promotion_type, &promotion.status, &promotion.image_url, &promotion.prize, &promotion.reward_points, &promotion.discount_percent, &promotion.max_claims, &promotion.per_user_limit, &promotion.total_claims, &promotion.requires_check_in, &promotion.requires_purchase, &promotion.terms, &promotion.metadata, &promotion.starts_at, &promotion.ends_at, &promotion.published_at, &promotion.created_by, &promotion.updated_by, &promotion.created_at, &promotion.updated_at],
        ).await?;

        Ok(row_to_business_promotion(&row))
    }

    pub async fn get_promotion(&self, promotion_id: Uuid) -> Result<Option<BusinessPromotion>, Error> {
        let client = self.pool.get().await?;
        let row = client.query_opt(
            "SELECT id, location_id, title, subtitle, description, promotion_type, status, image_url, prize, reward_points, discount_percent, max_claims, per_user_limit, total_claims, requires_check_in, requires_purchase, terms, metadata, starts_at, ends_at, published_at, created_by, updated_by, created_at, updated_at FROM business_promotions WHERE id = $1",
            &[&promotion_id],
        ).await?;

        Ok(row.map(|r| row_to_business_promotion(&r)))
    }

    pub async fn list_promotions_for_location(&self, location_id: Uuid) -> Result<Vec<BusinessPromotion>, Error> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT id, location_id, title, subtitle, description, promotion_type, status, image_url, prize, reward_points, discount_percent, max_claims, per_user_limit, total_claims, requires_check_in, requires_purchase, terms, metadata, starts_at, ends_at, published_at, created_by, updated_by, created_at, updated_at FROM business_promotions WHERE location_id = $1 ORDER BY starts_at DESC",
            &[&location_id],
        ).await?;

        Ok(rows.into_iter().map(|r| row_to_business_promotion(&r)).collect())
    }

    pub async fn list_promotions_for_business(&self, business_id: Uuid) -> Result<Vec<BusinessPromotion>, Error> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT p.id, p.location_id, p.title, p.subtitle, p.description, p.promotion_type, p.status, p.image_url, p.prize, p.reward_points, p.discount_percent, p.max_claims, p.per_user_limit, p.total_claims, p.requires_check_in, p.requires_purchase, p.terms, p.metadata, p.starts_at, p.ends_at, p.published_at, p.created_by, p.updated_by, p.created_at, p.updated_at FROM business_promotions p INNER JOIN business_locations l ON p.location_id = l.id WHERE l.business_id = $1 ORDER BY p.starts_at DESC",
            &[&business_id],
        ).await?;

        Ok(rows.into_iter().map(|r| row_to_business_promotion(&r)).collect())
    }

    pub async fn update_promotion(&self, promotion: BusinessPromotion) -> Result<BusinessPromotion, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "UPDATE business_promotions SET title = $2, subtitle = $3, description = $4, promotion_type = $5, status = $6, image_url = $7, prize = $8, reward_points = $9, discount_percent = $10, max_claims = $11, per_user_limit = $12, requires_check_in = $13, requires_purchase = $14, terms = $15, metadata = $16, starts_at = $17, ends_at = $18, published_at = $19, updated_by = $20, updated_at = NOW() WHERE id = $1 RETURNING id, location_id, title, subtitle, description, promotion_type, status, image_url, prize, reward_points, discount_percent, max_claims, per_user_limit, total_claims, requires_check_in, requires_purchase, terms, metadata, starts_at, ends_at, published_at, created_by, updated_by, created_at, updated_at",
            &[&promotion.id, &promotion.title, &promotion.subtitle, &promotion.description, &promotion.promotion_type, &promotion.status, &promotion.image_url, &promotion.prize, &promotion.reward_points, &promotion.discount_percent, &promotion.max_claims, &promotion.per_user_limit, &promotion.requires_check_in, &promotion.requires_purchase, &promotion.terms, &promotion.metadata, &promotion.starts_at, &promotion.ends_at, &promotion.published_at, &promotion.updated_by],
        ).await?;

        Ok(row_to_business_promotion(&row))
    }

    pub async fn delete_promotion(&self, promotion_id: Uuid) -> Result<(), Error> {
        let client = self.pool.get().await?;
        client.execute("DELETE FROM business_promotions WHERE id = $1", &[&promotion_id]).await?;
        Ok(())
    }

    pub async fn get_review_stats(&self) -> Result<ReviewStats, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "SELECT COUNT(*) FILTER (WHERE status = 'pending') AS pending, COUNT(*) FILTER (WHERE status = 'under_review') AS under_review, COUNT(*) FILTER (WHERE status = 'approved' AND submitted_at >= NOW() - INTERVAL '1 day') AS approved_today, COUNT(*) FILTER (WHERE status = 'rejected' AND submitted_at >= NOW() - INTERVAL '1 day') AS rejected_today FROM business_registration_requests",
            &[],
        ).await?;

        Ok(ReviewStats {
            pending: row.get("pending"),
            under_review: row.get("under_review"),
            approved_today: row.get("approved_today"),
            rejected_today: row.get("rejected_today"),
        })
    }

    pub async fn add_location_admin(&self, admin: NewLocationAdmin) -> Result<LocationAdmin, Error> {
        let client = self.pool.get().await?;
        let row = client.query_one(
            "INSERT INTO location_admins (id, location_id, user_id, user_email, user_username, role, granted_by, granted_by_username, is_active, granted_at, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING id, location_id, user_id, user_email, user_username, role, granted_by, granted_by_username, is_active, granted_at, created_at, updated_at",
            &[&admin.id, &admin.location_id, &admin.user_id, &admin.user_email, &admin.user_username, &admin.role, &admin.granted_by, &admin.granted_by_username, &admin.is_active, &admin.granted_at, &admin.created_at, &admin.updated_at],
        ).await?;

        Ok(row_to_location_admin(&row))
    }

    pub async fn list_location_admins(&self, location_id: Uuid) -> Result<Vec<LocationAdmin>, Error> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT id, location_id, user_id, user_email, user_username, role, granted_by, granted_by_username, is_active, granted_at, created_at, updated_at FROM location_admins WHERE location_id = $1 AND is_active = TRUE ORDER BY granted_at DESC",
            &[&location_id],
        ).await?;

        Ok(rows.into_iter().map(|r| row_to_location_admin(&r)).collect())
    }

    pub async fn remove_location_admin(&self, location_id: Uuid, user_id: Uuid) -> Result<(), Error> {
        let client = self.pool.get().await?;
        client.execute("UPDATE location_admins SET is_active = FALSE, updated_at = NOW() WHERE location_id = $1 AND user_id = $2", &[&location_id, &user_id]).await?;
        Ok(())
    }

    // For now, only the above are ported to demonstrate the pattern.
}

// Row mapping functions
fn row_to_location_admin(row: &tokio_postgres::Row) -> LocationAdmin {
    LocationAdmin {
        id: row.get("id"),
        location_id: row.get("location_id"),
        user_id: row.get("user_id"),
        user_email: row.get("user_email"),
        user_username: row.get("user_username"),
        role: row.get("role"),
        granted_by: row.get("granted_by"),
        granted_by_username: row.get("granted_by_username"),
        is_active: row.get("is_active"),
        granted_at: row.get("granted_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_business_promotion(row: &tokio_postgres::Row) -> BusinessPromotion {
    BusinessPromotion {
        id: row.get("id"),
        location_id: row.get("location_id"),
        title: row.get("title"),
        subtitle: row.get("subtitle"),
        description: row.get("description"),
        promotion_type: row.get("promotion_type"),
        status: row.get("status"),
        image_url: row.get("image_url"),
        prize: row.get("prize"),
        reward_points: row.get("reward_points"),
        discount_percent: row.get("discount_percent"),
        max_claims: row.get("max_claims"),
        per_user_limit: row.get("per_user_limit"),
        total_claims: row.get("total_claims"),
        requires_check_in: row.get("requires_check_in"),
        requires_purchase: row.get("requires_purchase"),
        terms: row.get("terms"),
        metadata: row.get("metadata"),
        starts_at: row.get("starts_at"),
        ends_at: row.get("ends_at"),
        published_at: row.get("published_at"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_business_location(row: &tokio_postgres::Row) -> BusinessLocation {
    BusinessLocation {
        id: row.get("id"),
        business_id: row.get("business_id"),
        location_name: row.get("location_name"),
        formatted_address: row.get("formatted_address"),
        street: row.get("street"),
        city: row.get("city"),
        state_region: row.get("state_region"),
        postal_code: row.get("postal_code"),
        country: row.get("country"),
        latitude: row.get("latitude"),
        longitude: row.get("longitude"),
        google_place_id: row.get("google_place_id"),
        timezone: row.get("timezone"),
        phone: row.get("phone"),
        email: row.get("email"),
        is_active: row.get("is_active"),
        is_primary: row.get("is_primary"),
        operating_hours: row.get("operating_hours"),
        notes: row.get("notes"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_business(row: &tokio_postgres::Row) -> Business {
    Business {
        id: row.get("id"),
        registration_id: row.get("registration_id"),
        owner_user_id: row.get("owner_user_id"),
        business_name: row.get("business_name"),
        category: row.get("category"),
        tax_id: row.get("tax_id"),
        description: row.get("description"),
        website: row.get("website"),
        logo_url: row.get("logo_url"),
        is_active: row.get("is_active"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_business_registration(row: &tokio_postgres::Row) -> BusinessRegistration {
    BusinessRegistration {
        id: row.get("id"),
        user_id: row.get("user_id"),
        business_id: row.get("business_id"),
        name: row.get("name"),
        category: row.get("category"),
        address: row.get("address"),
        description: row.get("description"),
        phone: row.get("phone"),
        website: row.get("website"),
        tax_id: row.get("tax_id"),
        document_urls: row.get("document_urls"),
        is_multi_user_team: row.get("is_multi_user_team"),
        status: row.get("status"),
        owner_email: row.get("owner_email"),
        owner_username: row.get("owner_username"),
        rejection_reason: row.get("rejection_reason"),
        reviewer_notes: row.get("reviewer_notes"),
        reviewer_id: row.get("reviewer_id"),
        reviewer_name: row.get("reviewer_name"),
        submitted_at: row.get("submitted_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_pending_business_review(row: &tokio_postgres::Row) -> PendingBusinessReview {
    PendingBusinessReview {
        id: row.get("id"),
        name: row.get("name"),
        category: row.get("category"),
        address: row.get("address"),
        tax_id: row.get("tax_id"),
        document_urls: row.get("document_urls"),
        submitted_at: row.get("submitted_at"),
        owner_email: row.get("owner_email"),
        owner_username: row.get("owner_username"),
    }
}