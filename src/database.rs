use std::{borrow::Cow, time::Duration};

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Connection, Executor, PgPool, Row,
};
use uuid::Uuid;

use crate::models::{
    Business, BusinessLocation, BusinessPromotion, BusinessRegistration, BusinessReviewEvent,
    BusinessVerificationStatus, LocationAdmin, NewBusiness, NewBusinessLocation,
    NewBusinessPromotion, NewBusinessRegistration, NewLocationAdmin, PendingBusinessReview,
    RegistrationWithHistory, ReviewAction, ReviewStats,
};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = match PgPoolOptions::new()
            .max_connections(10)
            .min_connections(2)
            .acquire_timeout(Duration::from_secs(5))
            .idle_timeout(Some(Duration::from_secs(600)))
            .test_before_acquire(true)
            .connect(database_url)
            .await
        {
            Ok(pool) => pool,
            Err(sqlx::Error::Database(db_err)) if db_err.code() == Some(Cow::Borrowed("3D000")) => {
                log::info!("Database missing, attempting to create it");
                create_database_if_missing(database_url).await?;

                PgPoolOptions::new()
                    .max_connections(10)
                    .min_connections(2)
                    .acquire_timeout(Duration::from_secs(5))
                    .idle_timeout(Some(Duration::from_secs(600)))
                    .test_before_acquire(true)
                    .connect(database_url)
                    .await?
            }
            Err(err) => return Err(err),
        };

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    // ========================================================================
    // BUSINESS REGISTRATION (Verification Workflow)
    // ========================================================================

    pub async fn create_registration(
        &self,
        registration: NewBusinessRegistration,
    ) -> Result<BusinessRegistration, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessRegistration>(
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
        )
        .bind(registration.id)
        .bind(registration.user_id)
        .bind(registration.business_id)
        .bind(registration.name)
        .bind(registration.category)
        .bind(registration.address)
        .bind(registration.description)
        .bind(registration.phone)
        .bind(registration.website)
        .bind(registration.tax_id)
        .bind(&registration.document_urls)
        .bind(registration.is_multi_user_team)
        .bind(registration.status)
        .bind(registration.owner_email)
        .bind(registration.owner_username)
        .bind(registration.rejection_reason)
        .bind(registration.reviewer_notes)
        .bind(registration.reviewer_id)
        .bind(registration.reviewer_name)
        .bind(registration.submitted_at)
        .bind(registration.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn get_registration_by_id(
        &self,
        registration_id: Uuid,
    ) -> Result<Option<BusinessRegistration>, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessRegistration>(
            "SELECT * FROM business_registration_requests WHERE id = $1",
        )
        .bind(registration_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn get_latest_registration_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Option<BusinessRegistration>, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessRegistration>(
            r#"
            SELECT * FROM business_registration_requests
            WHERE user_id = $1
            ORDER BY submitted_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn list_registrations_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<BusinessRegistration>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessRegistration>(
            r#"
            SELECT * FROM business_registration_requests
            WHERE user_id = $1
            ORDER BY submitted_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn list_pending_reviews(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PendingBusinessReview>, sqlx::Error> {
        let records = sqlx::query_as::<_, PendingBusinessReview>(
            r#"
            SELECT id, name, category, address, tax_id, document_urls,
                   submitted_at, owner_email, owner_username
            FROM business_registration_requests
            WHERE status = 'pending'
            ORDER BY submitted_at ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn record_review_event(
        &self,
        registration_id: Uuid,
        reviewer_id: Option<Uuid>,
        reviewer_name: Option<String>,
        action: ReviewAction,
        notes: Option<String>,
        rejection_reason: Option<String>,
        new_status: BusinessVerificationStatus,
    ) -> Result<BusinessRegistration, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO business_review_events (registration_id, reviewer_id, reviewer_name, action, notes, rejection_reason)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(registration_id)
        .bind(reviewer_id)
        .bind(&reviewer_name)
        .bind(action)
        .bind(&notes)
        .bind(&rejection_reason)
        .execute(tx.as_mut())
        .await?;

        // If approving, create the business record and get its ID
        let business_id = if new_status == BusinessVerificationStatus::Approved {
            // Get registration data to create business
            let registration = sqlx::query_as::<_, BusinessRegistration>(
                "SELECT * FROM business_registration_requests WHERE id = $1"
            )
            .bind(registration_id)
            .fetch_one(tx.as_mut())
            .await?;

            // Create business record
            let business = sqlx::query(
                r#"
                INSERT INTO businesses (registration_id, owner_user_id, business_name, category, tax_id, description, website, is_active)
                VALUES ($1, $2, $3, $4, $5, $6, $7, true)
                RETURNING id
                "#,
            )
            .bind(registration_id)
            .bind(&registration.user_id)
            .bind(&registration.name)
            .bind(&registration.category)
            .bind(&registration.tax_id)
            .bind(&registration.description)
            .bind(&registration.website)
            .fetch_one(tx.as_mut())
            .await?;

            Some(business.get::<Uuid, _>("id"))
        } else {
            None
        };

        let updated = sqlx::query_as::<_, BusinessRegistration>(
            r#"
            UPDATE business_registration_requests
            SET status = $2, business_id = $3, reviewer_id = $4, reviewer_name = $5,
                reviewer_notes = $6, rejection_reason = $7, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(registration_id)
        .bind(new_status)
        .bind(business_id)
        .bind(reviewer_id)
        .bind(reviewer_name)
        .bind(notes)
        .bind(rejection_reason)
        .fetch_one(tx.as_mut())
        .await?;

        tx.commit().await?;

        Ok(updated)
    }

    pub async fn list_review_events(
        &self,
        registration_id: Uuid,
    ) -> Result<Vec<BusinessReviewEvent>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessReviewEvent>(
            r#"
            SELECT * FROM business_review_events
            WHERE registration_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(registration_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn get_registration_with_history(
        &self,
        registration_id: Uuid,
    ) -> Result<Option<RegistrationWithHistory>, sqlx::Error> {
        let registration = match self.get_registration_by_id(registration_id).await? {
            Some(r) => r,
            None => return Ok(None),
        };

        let history = self.list_review_events(registration_id).await?;

        Ok(Some(RegistrationWithHistory {
            registration,
            history,
        }))
    }

    pub async fn get_review_stats(&self) -> Result<ReviewStats, sqlx::Error> {
        let record = sqlx::query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'pending') as pending,
                COUNT(*) FILTER (WHERE status = 'under_review') as under_review,
                COUNT(*) FILTER (WHERE status = 'approved' AND DATE(updated_at) = CURRENT_DATE) as approved_today,
                COUNT(*) FILTER (WHERE status = 'rejected' AND DATE(updated_at) = CURRENT_DATE) as rejected_today
            FROM business_registration_requests
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(ReviewStats {
            pending: record.try_get("pending")?,
            under_review: record.try_get("under_review")?,
            approved_today: record.try_get("approved_today")?,
            rejected_today: record.try_get("rejected_today")?,
        })
    }

    // ========================================================================
    // BUSINESSES
    // ========================================================================

    pub async fn create_business(
        &self,
        business: NewBusiness,
    ) -> Result<Business, sqlx::Error> {
        let record = sqlx::query_as::<_, Business>(
            r#"
            INSERT INTO businesses (
                id, registration_id, owner_user_id, business_name, category,
                tax_id, description, website, logo_url, is_active, metadata,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(business.id)
        .bind(business.registration_id)
        .bind(business.owner_user_id)
        .bind(business.business_name)
        .bind(business.category)
        .bind(business.tax_id)
        .bind(business.description)
        .bind(business.website)
        .bind(business.logo_url)
        .bind(business.is_active)
        .bind(business.metadata)
        .bind(business.created_at)
        .bind(business.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn get_business(&self, business_id: Uuid) -> Result<Option<Business>, sqlx::Error> {
        let record = sqlx::query_as::<_, Business>("SELECT * FROM businesses WHERE id = $1")
            .bind(business_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(record)
    }

    pub async fn list_businesses_for_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<Business>, sqlx::Error> {
        let records = sqlx::query_as::<_, Business>(
            r#"
            SELECT * FROM businesses
            WHERE owner_user_id = $1 AND is_active = TRUE
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn update_business(&self, business: Business) -> Result<Business, sqlx::Error> {
        let record = sqlx::query_as::<_, Business>(
            r#"
            UPDATE businesses
            SET business_name = $2, category = $3, tax_id = $4, description = $5,
                website = $6, logo_url = $7, is_active = $8, metadata = $9, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(business.id)
        .bind(business.business_name)
        .bind(business.category)
        .bind(business.tax_id)
        .bind(business.description)
        .bind(business.website)
        .bind(business.logo_url)
        .bind(business.is_active)
        .bind(business.metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn delete_business(&self, business_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM businesses WHERE id = $1")
            .bind(business_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ========================================================================
    // BUSINESS LOCATIONS
    // ========================================================================

    pub async fn create_location(
        &self,
        location: NewBusinessLocation,
    ) -> Result<BusinessLocation, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        if location.is_primary {
            sqlx::query("UPDATE business_locations SET is_primary = FALSE WHERE business_id = $1")
                .bind(location.business_id)
                .execute(tx.as_mut())
                .await?;
        }

        let record = sqlx::query_as::<_, BusinessLocation>(
            r#"
            INSERT INTO business_locations (
                id, business_id, location_name, formatted_address, street, city,
                state_region, postal_code, country, latitude, longitude, google_place_id,
                timezone, phone, email, is_active, is_primary, operating_hours, notes, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            RETURNING *
            "#,
        )
        .bind(location.id)
        .bind(location.business_id)
        .bind(location.location_name)
        .bind(location.formatted_address)
        .bind(location.street)
        .bind(location.city)
        .bind(location.state_region)
        .bind(location.postal_code)
        .bind(location.country)
        .bind(location.latitude)
        .bind(location.longitude)
        .bind(location.google_place_id)
        .bind(location.timezone)
        .bind(location.phone)
        .bind(location.email)
        .bind(location.is_active)
        .bind(location.is_primary)
        .bind(location.operating_hours)
        .bind(location.notes)
        .bind(location.metadata)
        .fetch_one(tx.as_mut())
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    pub async fn get_location(
        &self,
        location_id: Uuid,
    ) -> Result<Option<BusinessLocation>, sqlx::Error> {
        let record =
            sqlx::query_as::<_, BusinessLocation>("SELECT * FROM business_locations WHERE id = $1")
                .bind(location_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(record)
    }

    pub async fn list_locations_for_business(
        &self,
        business_id: Uuid,
    ) -> Result<Vec<BusinessLocation>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessLocation>(
            r#"
            SELECT * FROM business_locations
            WHERE business_id = $1 AND is_active = TRUE
            ORDER BY is_primary DESC, created_at ASC
            "#,
        )
        .bind(business_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn update_location(
        &self,
        location: BusinessLocation,
    ) -> Result<BusinessLocation, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        if location.is_primary {
            sqlx::query(
                "UPDATE business_locations SET is_primary = FALSE WHERE business_id = $1 AND id != $2",
            )
            .bind(location.business_id)
            .bind(location.id)
            .execute(tx.as_mut())
            .await?;
        }

        let record = sqlx::query_as::<_, BusinessLocation>(
            r#"
            UPDATE business_locations
            SET location_name = $2, formatted_address = $3, street = $4, city = $5,
                state_region = $6, postal_code = $7, country = $8, latitude = $9,
                longitude = $10, google_place_id = $11, timezone = $12, phone = $13,
                email = $14, is_active = $15, is_primary = $16, operating_hours = $17,
                notes = $18, metadata = $19, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(location.id)
        .bind(location.location_name)
        .bind(location.formatted_address)
        .bind(location.street)
        .bind(location.city)
        .bind(location.state_region)
        .bind(location.postal_code)
        .bind(location.country)
        .bind(location.latitude)
        .bind(location.longitude)
        .bind(location.google_place_id)
        .bind(location.timezone)
        .bind(location.phone)
        .bind(location.email)
        .bind(location.is_active)
        .bind(location.is_primary)
        .bind(location.operating_hours)
        .bind(location.notes)
        .bind(location.metadata)
        .fetch_one(tx.as_mut())
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    pub async fn delete_location(&self, location_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM business_locations WHERE id = $1")
            .bind(location_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ========================================================================
    // BUSINESS PROMOTIONS
    // ========================================================================

    pub async fn create_promotion(
        &self,
        promotion: NewBusinessPromotion,
    ) -> Result<BusinessPromotion, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessPromotion>(
            r#"
            INSERT INTO business_promotions (
                id, location_id, title, subtitle, description, promotion_type, status,
                image_url, prize, reward_points, discount_percent, max_claims,
                per_user_limit, total_claims, requires_check_in, requires_purchase,
                terms, metadata, starts_at, ends_at, published_at, created_by,
                updated_by, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25)
            RETURNING *
            "#,
        )
        .bind(promotion.id)
        .bind(promotion.location_id)
        .bind(promotion.title)
        .bind(promotion.subtitle)
        .bind(promotion.description)
        .bind(promotion.promotion_type)
        .bind(promotion.status)
        .bind(promotion.image_url)
        .bind(promotion.prize)
        .bind(promotion.reward_points)
        .bind(promotion.discount_percent)
        .bind(promotion.max_claims)
        .bind(promotion.per_user_limit)
        .bind(promotion.total_claims)
        .bind(promotion.requires_check_in)
        .bind(promotion.requires_purchase)
        .bind(promotion.terms)
        .bind(promotion.metadata)
        .bind(promotion.starts_at)
        .bind(promotion.ends_at)
        .bind(promotion.published_at)
        .bind(promotion.created_by)
        .bind(promotion.updated_by)
        .bind(promotion.created_at)
        .bind(promotion.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn get_promotion(
        &self,
        promotion_id: Uuid,
    ) -> Result<Option<BusinessPromotion>, sqlx::Error> {
        let record =
            sqlx::query_as::<_, BusinessPromotion>("SELECT * FROM business_promotions WHERE id = $1")
                .bind(promotion_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(record)
    }

    pub async fn list_promotions_for_location(
        &self,
        location_id: Uuid,
    ) -> Result<Vec<BusinessPromotion>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessPromotion>(
            r#"
            SELECT * FROM business_promotions
            WHERE location_id = $1
            ORDER BY starts_at DESC
            "#,
        )
        .bind(location_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn list_promotions_for_business(
        &self,
        business_id: Uuid,
    ) -> Result<Vec<BusinessPromotion>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessPromotion>(
            r#"
            SELECT p.* FROM business_promotions p
            INNER JOIN business_locations l ON p.location_id = l.id
            WHERE l.business_id = $1
            ORDER BY p.starts_at DESC
            "#,
        )
        .bind(business_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn update_promotion(
        &self,
        promotion: BusinessPromotion,
    ) -> Result<BusinessPromotion, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessPromotion>(
            r#"
            UPDATE business_promotions
            SET title = $2, subtitle = $3, description = $4, promotion_type = $5,
                status = $6, image_url = $7, prize = $8, reward_points = $9,
                discount_percent = $10, max_claims = $11, per_user_limit = $12,
                requires_check_in = $13, requires_purchase = $14, terms = $15,
                metadata = $16, starts_at = $17, ends_at = $18, published_at = $19,
                updated_by = $20, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(promotion.id)
        .bind(promotion.title)
        .bind(promotion.subtitle)
        .bind(promotion.description)
        .bind(promotion.promotion_type)
        .bind(promotion.status)
        .bind(promotion.image_url)
        .bind(promotion.prize)
        .bind(promotion.reward_points)
        .bind(promotion.discount_percent)
        .bind(promotion.max_claims)
        .bind(promotion.per_user_limit)
        .bind(promotion.requires_check_in)
        .bind(promotion.requires_purchase)
        .bind(promotion.terms)
        .bind(promotion.metadata)
        .bind(promotion.starts_at)
        .bind(promotion.ends_at)
        .bind(promotion.published_at)
        .bind(promotion.updated_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn delete_promotion(&self, promotion_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM business_promotions WHERE id = $1")
            .bind(promotion_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // ========================================================================
    // LOCATION ADMINISTRATORS
    // ========================================================================

    pub async fn add_location_admin(
        &self,
        admin: NewLocationAdmin,
    ) -> Result<LocationAdmin, sqlx::Error> {
        let record = sqlx::query_as::<_, LocationAdmin>(
            r#"
            INSERT INTO business_location_admins (
                id, location_id, user_id, user_email, user_username, role,
                granted_by, granted_by_username, is_active, granted_at,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING *
            "#,
        )
        .bind(admin.id)
        .bind(admin.location_id)
        .bind(admin.user_id)
        .bind(admin.user_email)
        .bind(admin.user_username)
        .bind(admin.role)
        .bind(admin.granted_by)
        .bind(admin.granted_by_username)
        .bind(admin.is_active)
        .bind(admin.granted_at)
        .bind(admin.created_at)
        .bind(admin.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn list_location_admins(
        &self,
        location_id: Uuid,
    ) -> Result<Vec<LocationAdmin>, sqlx::Error> {
        let records = sqlx::query_as::<_, LocationAdmin>(
            r#"
            SELECT * FROM business_location_admins
            WHERE location_id = $1 AND is_active = TRUE
            ORDER BY granted_at ASC
            "#,
        )
        .bind(location_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn remove_location_admin(
        &self,
        location_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE business_location_admins SET is_active = FALSE WHERE location_id = $1 AND user_id = $2",
        )
        .bind(location_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn check_user_is_location_admin(
        &self,
        location_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM business_location_admins
            WHERE location_id = $1 AND user_id = $2 AND is_active = TRUE
            "#,
        )
        .bind(location_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }
}

async fn create_database_if_missing(database_url: &str) -> Result<(), sqlx::Error> {
    let options: PgConnectOptions = database_url.parse()?;
    let database_name = options
        .get_database()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "postgres".to_string());

    if database_name.eq_ignore_ascii_case("postgres") {
        return Ok(());
    }

    let maintenance_options = options.clone().database("postgres");
    let mut connection = sqlx::postgres::PgConnection::connect_with(&maintenance_options).await?;

    let escaped_name = database_name.replace('"', "\"");
    let create_stmt = format!("CREATE DATABASE \"{}\"", escaped_name);

    match connection.execute(create_stmt.as_str()).await {
        Ok(_) => {
            log::info!("Created database '{}'", database_name);
            Ok(())
        }
        Err(sqlx::Error::Database(db_err)) if db_err.code() == Some(Cow::Borrowed("42P04")) => {
            log::info!("Database '{}' already exists", database_name);
            Ok(())
        }
        Err(err) => Err(err),
    }
}
