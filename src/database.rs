use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    time::Duration,
};

use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Connection, Executor, PgPool, Postgres, Row, Transaction,
};
use uuid::Uuid;

use crate::models::{
    BusinessCompany, BusinessLocation, BusinessPromotion, BusinessPromotionScope,
    BusinessPromotionWithLocations, BusinessRegistration, BusinessRegistrationSummary,
    BusinessReviewEvent, BusinessUnit, BusinessUnitDetail, BusinessVerificationStatus,
    CompanyWithUnits, NewBusinessLocation, NewBusinessPromotion, NewBusinessRegistration,
    PendingBusinessReview, ReviewAction, ReviewStats,
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

        // Run embedded migrations
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn create_registration(
        &self,
        registration: NewBusinessRegistration,
        locations: Vec<NewBusinessLocation>,
    ) -> Result<(BusinessRegistration, Vec<BusinessLocation>), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let NewBusinessRegistration {
            id,
            user_id,
            business_id,
            name,
            category,
            address,
            description,
            phone,
            website,
            tax_id,
            document_urls,
            is_multi_user_team,
            status,
            owner_email,
            owner_username,
            rejection_reason,
            reviewer_notes,
            reviewer_id,
            reviewer_name,
            submitted_at,
            updated_at,
        } = registration;

        let record = {
            let conn = tx.as_mut();
            sqlx::query_as::<_, BusinessRegistration>(
                r#"
                INSERT INTO business_registration_requests (
                    id,
                    user_id,
                    business_id,
                    name,
                    category,
                    address,
                    description,
                    phone,
                    website,
                    tax_id,
                    document_urls,
                    is_multi_user_team,
                    status,
                    owner_email,
                    owner_username,
                    rejection_reason,
                    reviewer_notes,
                    reviewer_id,
                    reviewer_name,
                    submitted_at,
                    updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                    $21
                )
                RETURNING
                    id,
                    user_id,
                    business_id,
                    name,
                    category,
                    address,
                    description,
                    phone,
                    website,
                    tax_id,
                    document_urls,
                    is_multi_user_team,
                    status,
                    owner_email,
                    owner_username,
                    rejection_reason,
                    reviewer_notes,
                    reviewer_id,
                    reviewer_name,
                    submitted_at,
                    updated_at
                "#,
            )
            .bind(id)
            .bind(user_id)
            .bind(business_id)
            .bind(name)
            .bind(category)
            .bind(address)
            .bind(description)
            .bind(phone)
            .bind(website)
            .bind(tax_id)
            .bind(document_urls)
            .bind(is_multi_user_team)
            .bind(status)
            .bind(owner_email)
            .bind(owner_username)
            .bind(rejection_reason)
            .bind(reviewer_notes)
            .bind(reviewer_id)
            .bind(reviewer_name)
            .bind(submitted_at)
            .bind(updated_at)
            .fetch_one(conn)
            .await?
        };

        let mut stored_locations = Vec::with_capacity(locations.len());
        for location in locations {
            let inserted = Self::insert_location_with_tx(&mut tx, location).await?;
            stored_locations.push(inserted);
        }

        tx.commit().await?;

        Ok((record, stored_locations))
    }
    #[allow(dead_code)]
    pub async fn upsert_business_id(
        &self,
        registration_id: uuid::Uuid,
        business_id: uuid::Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE business_registration_requests
            SET business_id = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(registration_id)
        .bind(business_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_registration_by_id(
        &self,
        registration_id: uuid::Uuid,
    ) -> Result<Option<BusinessRegistration>, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessRegistration>(
            r#"
            SELECT
                id,
                user_id,
                business_id,
                name,
                category,
                address,
                description,
                phone,
                website,
                tax_id,
                document_urls,
                is_multi_user_team,
                status,
                owner_email,
                owner_username,
                rejection_reason,
                reviewer_notes,
                reviewer_id,
                reviewer_name,
                submitted_at,
                updated_at
            FROM business_registration_requests
            WHERE id = $1
            "#,
        )
        .bind(registration_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn get_latest_registration_for_user(
        &self,
        user_id: uuid::Uuid,
    ) -> Result<Option<BusinessRegistration>, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessRegistration>(
            r#"
            SELECT
                id,
                user_id,
                business_id,
                name,
                category,
                address,
                description,
                phone,
                website,
                tax_id,
                document_urls,
                is_multi_user_team,
                status,
                owner_email,
                owner_username,
                rejection_reason,
                reviewer_notes,
                reviewer_id,
                reviewer_name,
                submitted_at,
                updated_at
            FROM business_registration_requests
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
    ) -> Result<Vec<BusinessRegistrationSummary>, sqlx::Error> {
        let registrations = sqlx::query_as::<_, BusinessRegistration>(
            r#"
            SELECT
                id,
                user_id,
                business_id,
                name,
                category,
                address,
                description,
                phone,
                website,
                tax_id,
                document_urls,
                is_multi_user_team,
                status,
                owner_email,
                owner_username,
                rejection_reason,
                reviewer_notes,
                reviewer_id,
                reviewer_name,
                submitted_at,
                updated_at
            FROM business_registration_requests
            WHERE user_id = $1
            ORDER BY submitted_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        if registrations.is_empty() {
            return Ok(Vec::new());
        }

        let registration_ids: Vec<Uuid> = registrations.iter().map(|reg| reg.id).collect();
        let locations = self
            .fetch_locations_for_registrations(&registration_ids)
            .await?;

        let mut grouped_locations: HashMap<Uuid, Vec<BusinessLocation>> = HashMap::new();
        for location in locations {
            grouped_locations
                .entry(location.registration_id)
                .or_default()
                .push(location);
        }

        let summaries = registrations
            .into_iter()
            .map(|registration| {
                let locations = grouped_locations
                    .remove(&registration.id)
                    .unwrap_or_default();
                BusinessRegistrationSummary {
                    registration,
                    locations,
                }
            })
            .collect();

        Ok(summaries)
    }

    async fn fetch_locations_for_registrations(
        &self,
        registration_ids: &[Uuid],
    ) -> Result<Vec<BusinessLocation>, sqlx::Error> {
        if registration_ids.is_empty() {
            return Ok(Vec::new());
        }

        let records = sqlx::query_as::<_, BusinessLocation>(
            r#"
            SELECT
                id,
                registration_id,
                business_id,
                label,
                formatted_address,
                street,
                city,
                state_region,
                postal_code,
                country,
                latitude,
                longitude,
                google_place_id,
                timezone,
                phone,
                is_primary,
                notes,
                metadata,
                created_at,
                updated_at
            FROM business_locations
            WHERE registration_id = ANY($1)
            ORDER BY registration_id, is_primary DESC, created_at ASC
            "#,
        )
        .bind(&registration_ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    async fn fetch_locations_for_promotions(
        &self,
        promotion_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<BusinessLocation>>, sqlx::Error> {
        if promotion_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT bpl.promotion_id,
                   bl.id,
                   bl.registration_id,
                   bl.business_id,
                   bl.label,
                   bl.formatted_address,
                   bl.street,
                   bl.city,
                   bl.state_region,
                   bl.postal_code,
                   bl.country,
                   bl.latitude,
                   bl.longitude,
                   bl.google_place_id,
                   bl.timezone,
                   bl.phone,
                   bl.is_primary,
                   bl.notes,
                   bl.metadata,
                   bl.created_at,
                   bl.updated_at
            FROM business_promotion_locations bpl
            INNER JOIN business_locations bl ON bl.id = bpl.location_id
            WHERE bpl.promotion_id = ANY($1)
            ORDER BY bpl.promotion_id, bl.is_primary DESC, bl.created_at ASC
            "#,
        )
        .bind(promotion_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut map: HashMap<Uuid, Vec<BusinessLocation>> = HashMap::new();

        for row in rows {
            let promotion_id: Uuid = row.try_get("promotion_id")?;
            let location = BusinessLocation {
                id: row.try_get("id")?,
                registration_id: row.try_get("registration_id")?,
                business_id: row.try_get("business_id")?,
                label: row.try_get("label")?,
                formatted_address: row.try_get("formatted_address")?,
                street: row.try_get("street")?,
                city: row.try_get("city")?,
                state_region: row.try_get("state_region")?,
                postal_code: row.try_get("postal_code")?,
                country: row.try_get("country")?,
                latitude: row.try_get("latitude")?,
                longitude: row.try_get("longitude")?,
                google_place_id: row.try_get("google_place_id")?,
                timezone: row.try_get("timezone")?,
                phone: row.try_get("phone")?,
                is_primary: row.try_get("is_primary")?,
                notes: row.try_get("notes")?,
                metadata: row.try_get("metadata")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };

            map.entry(promotion_id).or_default().push(location);
        }

        Ok(map)
    }

    pub async fn list_locations_for_registration(
        &self,
        registration_id: Uuid,
    ) -> Result<Vec<BusinessLocation>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessLocation>(
            r#"
            SELECT
                id,
                registration_id,
                business_id,
                label,
                formatted_address,
                street,
                city,
                state_region,
                postal_code,
                country,
                latitude,
                longitude,
                google_place_id,
                timezone,
                phone,
                is_primary,
                notes,
                metadata,
                created_at,
                updated_at
            FROM business_locations
            WHERE registration_id = $1
            ORDER BY is_primary DESC, created_at ASC
            "#,
        )
        .bind(registration_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn create_location_for_registration(
        &self,
        location: NewBusinessLocation,
    ) -> Result<BusinessLocation, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let inserted = Self::insert_location_with_tx(&mut tx, location).await?;
        tx.commit().await?;
        Ok(inserted)
    }

    pub async fn get_location_by_id(
        &self,
        registration_id: Uuid,
        location_id: Uuid,
    ) -> Result<Option<BusinessLocation>, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessLocation>(
            r#"
            SELECT
                id,
                registration_id,
                business_id,
                label,
                formatted_address,
                street,
                city,
                state_region,
                postal_code,
                country,
                latitude,
                longitude,
                google_place_id,
                timezone,
                phone,
                is_primary,
                notes,
                metadata,
                created_at,
                updated_at
            FROM business_locations
            WHERE registration_id = $1 AND id = $2
            "#,
        )
        .bind(registration_id)
        .bind(location_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn update_location(
        &self,
        location: BusinessLocation,
    ) -> Result<BusinessLocation, sqlx::Error> {
        let BusinessLocation {
            id,
            registration_id,
            business_id,
            label,
            formatted_address,
            street,
            city,
            state_region,
            postal_code,
            country,
            latitude,
            longitude,
            google_place_id,
            timezone,
            phone,
            is_primary,
            notes,
            metadata,
            created_at: _,
            updated_at: _,
        } = location;

        let mut tx = self.pool.begin().await?;

        if is_primary {
            let conn = tx.as_mut();
            sqlx::query(
                r#"
                UPDATE business_locations
                SET is_primary = FALSE
                WHERE registration_id = $1 AND id <> $2
                "#,
            )
            .bind(registration_id)
            .bind(id)
            .execute(conn)
            .await?;
        }

        let updated = {
            let conn = tx.as_mut();
            sqlx::query_as::<_, BusinessLocation>(
                r#"
                UPDATE business_locations
                SET
                    business_id = $3,
                    label = $4,
                    formatted_address = $5,
                    street = $6,
                    city = $7,
                    state_region = $8,
                    postal_code = $9,
                    country = $10,
                    latitude = $11,
                    longitude = $12,
                    google_place_id = $13,
                    timezone = $14,
                    phone = $15,
                    is_primary = $16,
                    notes = $17,
                    metadata = $18,
                    updated_at = NOW()
                WHERE registration_id = $1 AND id = $2
                RETURNING
                    id,
                    registration_id,
                    business_id,
                    label,
                    formatted_address,
                    street,
                    city,
                    state_region,
                    postal_code,
                    country,
                    latitude,
                    longitude,
                    google_place_id,
                    timezone,
                    phone,
                    is_primary,
                    notes,
                    metadata,
                    created_at,
                    updated_at
                "#,
            )
            .bind(registration_id)
            .bind(id)
            .bind(business_id)
            .bind(label)
            .bind(formatted_address)
            .bind(street)
            .bind(city)
            .bind(state_region)
            .bind(postal_code)
            .bind(country)
            .bind(latitude)
            .bind(longitude)
            .bind(google_place_id)
            .bind(timezone)
            .bind(phone)
            .bind(is_primary)
            .bind(notes)
            .bind(metadata)
            .fetch_one(conn)
            .await?
        };

        tx.commit().await?;

        Ok(updated)
    }

    pub async fn delete_location(
        &self,
        registration_id: Uuid,
        location_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM business_locations
            WHERE registration_id = $1 AND id = $2
            "#,
        )
        .bind(registration_id)
        .bind(location_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    async fn insert_location_with_tx(
        tx: &mut Transaction<'_, Postgres>,
        location: NewBusinessLocation,
    ) -> Result<BusinessLocation, sqlx::Error> {
        let NewBusinessLocation {
            id,
            registration_id,
            business_id,
            label,
            formatted_address,
            street,
            city,
            state_region,
            postal_code,
            country,
            latitude,
            longitude,
            google_place_id,
            timezone,
            phone,
            is_primary,
            notes,
            metadata,
        } = location;

        if is_primary {
            let conn = tx.as_mut();
            sqlx::query(
                r#"
                UPDATE business_locations
                SET is_primary = FALSE
                WHERE registration_id = $1
                "#,
            )
            .bind(registration_id)
            .execute(conn)
            .await?;
        }

        let record = {
            let conn = tx.as_mut();
            sqlx::query_as::<_, BusinessLocation>(
                r#"
                INSERT INTO business_locations (
                    id,
                    registration_id,
                    business_id,
                    label,
                    formatted_address,
                    street,
                    city,
                    state_region,
                    postal_code,
                    country,
                    latitude,
                    longitude,
                    google_place_id,
                    timezone,
                    phone,
                    is_primary,
                    notes,
                    metadata
                )
                VALUES (
                    $1, $2, $3, $4, $5,
                    $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15,
                    $16, $17, $18
                )
                RETURNING
                    id,
                    registration_id,
                    business_id,
                    label,
                    formatted_address,
                    street,
                    city,
                    state_region,
                    postal_code,
                    country,
                    latitude,
                    longitude,
                    google_place_id,
                    timezone,
                    phone,
                    is_primary,
                    notes,
                    metadata,
                    created_at,
                    updated_at
                "#,
            )
            .bind(id)
            .bind(registration_id)
            .bind(business_id)
            .bind(label)
            .bind(formatted_address)
            .bind(street)
            .bind(city)
            .bind(state_region)
            .bind(postal_code)
            .bind(country)
            .bind(latitude)
            .bind(longitude)
            .bind(google_place_id)
            .bind(timezone)
            .bind(phone)
            .bind(is_primary)
            .bind(notes)
            .bind(metadata)
            .fetch_one(conn)
            .await?
        };

        Ok(record)
    }

    fn dedupe_uuids(ids: &[Uuid]) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        ids.iter().cloned().filter(|id| seen.insert(*id)).collect()
    }

    async fn sync_promotion_locations(
        tx: &mut Transaction<'_, Postgres>,
        registration_id: Uuid,
        promotion_id: Uuid,
        location_ids: &[Uuid],
    ) -> Result<(), sqlx::Error> {
        if location_ids.is_empty() {
            return Ok(());
        }

        let rows = {
            let conn = tx.as_mut();
            sqlx::query(
                r#"
                SELECT id
                FROM business_locations
                WHERE registration_id = $1 AND id = ANY($2)
                "#,
            )
            .bind(registration_id)
            .bind(location_ids)
            .fetch_all(conn)
            .await?
        };

        if rows.len() != location_ids.len() {
            return Err(sqlx::Error::RowNotFound);
        }

        for location_id in location_ids {
            let conn = tx.as_mut();
            sqlx::query(
                r#"
                INSERT INTO business_promotion_locations (promotion_id, location_id)
                VALUES ($1, $2)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(promotion_id)
            .bind(location_id)
            .execute(conn)
            .await?;
        }

        Ok(())
    }

    pub async fn create_promotion(
        &self,
        promotion: NewBusinessPromotion,
        location_ids: &[Uuid],
    ) -> Result<BusinessPromotionWithLocations, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let NewBusinessPromotion {
            id,
            registration_id,
            unit_id,
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
            total_claims,
            requires_check_in,
            requires_purchase,
            terms,
            metadata,
            starts_at,
            ends_at,
            published_at,
            created_by,
            updated_by,
            created_at,
            updated_at,
        } = promotion;

        let inserted = {
            let conn = tx.as_mut();
            sqlx::query_as::<_, BusinessPromotion>(
                r#"
                INSERT INTO business_promotions (
                    id,
                    registration_id,
                    unit_id,
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
                    total_claims,
                    requires_check_in,
                    requires_purchase,
                    terms,
                    metadata,
                    starts_at,
                    ends_at,
                    published_at,
                    created_by,
                    updated_by,
                    created_at,
                    updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5,
                    $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15,
                    $16, $17, $18, $19, $20,
                    $21, $22, $23, $24, $25,
                    $26, $27
                )
                RETURNING
                    id,
                    registration_id,
                    unit_id,
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
                    total_claims,
                    requires_check_in,
                    requires_purchase,
                    terms,
                    metadata,
                    starts_at,
                    ends_at,
                    published_at,
                    created_by,
                    updated_by,
                    created_at,
                    updated_at
                "#,
            )
            .bind(id)
            .bind(registration_id)
            .bind(unit_id)
            .bind(title)
            .bind(subtitle)
            .bind(description)
            .bind(promotion_type)
            .bind(scope)
            .bind(status)
            .bind(image_url)
            .bind(prize)
            .bind(reward_points)
            .bind(discount_percent)
            .bind(max_claims)
            .bind(per_user_limit)
            .bind(total_claims)
            .bind(requires_check_in)
            .bind(requires_purchase)
            .bind(terms)
            .bind(metadata)
            .bind(starts_at)
            .bind(ends_at)
            .bind(published_at)
            .bind(created_by)
            .bind(updated_by)
            .bind(created_at)
            .bind(updated_at)
            .fetch_one(conn)
            .await?
        };

        if inserted.scope == BusinessPromotionScope::Location {
            let unique_location_ids = Self::dedupe_uuids(location_ids);
            Self::sync_promotion_locations(
                &mut tx,
                inserted.registration_id,
                inserted.id,
                &unique_location_ids,
            )
            .await?;
        }

        tx.commit().await?;

        let mut location_map = self.fetch_locations_for_promotions(&[inserted.id]).await?;

        let locations = location_map.remove(&inserted.id).unwrap_or_default();

        Ok(BusinessPromotionWithLocations {
            promotion: inserted,
            locations,
        })
    }

    pub async fn update_promotion(
        &self,
        promotion: BusinessPromotion,
        location_ids: &[Uuid],
    ) -> Result<BusinessPromotionWithLocations, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let updated = {
            let conn = tx.as_mut();
            sqlx::query_as::<_, BusinessPromotion>(
                r#"
                UPDATE business_promotions
                SET
                    title = $3,
                    subtitle = $4,
                    description = $5,
                    promotion_type = $6,
                    scope = $7,
                    status = $8,
                    image_url = $9,
                    prize = $10,
                    reward_points = $11,
                    discount_percent = $12,
                    max_claims = $13,
                    per_user_limit = $14,
                    requires_check_in = $15,
                    requires_purchase = $16,
                    terms = $17,
                    metadata = $18,
                    starts_at = $19,
                    ends_at = $20,
                    published_at = $21,
                    updated_by = $22,
                    updated_at = $23
                WHERE id = $1 AND registration_id = $2
                RETURNING
                    id,
                    registration_id,
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
                    total_claims,
                    requires_check_in,
                    requires_purchase,
                    terms,
                    metadata,
                    starts_at,
                    ends_at,
                    published_at,
                    created_by,
                    updated_by,
                    created_at,
                    updated_at
                "#,
            )
            .bind(promotion.id)
            .bind(promotion.registration_id)
            .bind(promotion.title)
            .bind(promotion.subtitle)
            .bind(promotion.description)
            .bind(promotion.promotion_type)
            .bind(promotion.scope)
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
            .bind(promotion.updated_at)
            .fetch_one(conn)
            .await?
        };

        {
            let conn = tx.as_mut();
            sqlx::query(
                r#"
                DELETE FROM business_promotion_locations
                WHERE promotion_id = $1
                "#,
            )
            .bind(updated.id)
            .execute(conn)
            .await?;
        }

        if updated.scope == BusinessPromotionScope::Location {
            let unique_location_ids = Self::dedupe_uuids(location_ids);
            Self::sync_promotion_locations(
                &mut tx,
                updated.registration_id,
                updated.id,
                &unique_location_ids,
            )
            .await?;
        }

        tx.commit().await?;

        let mut location_map = self.fetch_locations_for_promotions(&[updated.id]).await?;

        let locations = location_map.remove(&updated.id).unwrap_or_default();

        Ok(BusinessPromotionWithLocations {
            promotion: updated,
            locations,
        })
    }

    pub async fn delete_promotion(
        &self,
        registration_id: Uuid,
        promotion_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM business_promotions
            WHERE id = $1 AND registration_id = $2
            "#,
        )
        .bind(promotion_id)
        .bind(registration_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }

        Ok(())
    }

    pub async fn get_promotion_with_locations(
        &self,
        registration_id: Uuid,
        promotion_id: Uuid,
    ) -> Result<Option<BusinessPromotionWithLocations>, sqlx::Error> {
        let promotion = sqlx::query_as::<_, BusinessPromotion>(
            r#"
            SELECT
                id,
                registration_id,
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
                total_claims,
                requires_check_in,
                requires_purchase,
                terms,
                metadata,
                starts_at,
                ends_at,
                published_at,
                created_by,
                updated_by,
                created_at,
                updated_at
            FROM business_promotions
            WHERE id = $1 AND registration_id = $2
            "#,
        )
        .bind(promotion_id)
        .bind(registration_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(promotion) = promotion else {
            return Ok(None);
        };

        let mut location_map = self.fetch_locations_for_promotions(&[promotion.id]).await?;

        let locations = location_map.remove(&promotion.id).unwrap_or_default();

        Ok(Some(BusinessPromotionWithLocations {
            promotion,
            locations,
        }))
    }

    pub async fn list_promotions_for_registration(
        &self,
        registration_id: Uuid,
    ) -> Result<Vec<BusinessPromotionWithLocations>, sqlx::Error> {
        let promotions = sqlx::query_as::<_, BusinessPromotion>(
            r#"
            SELECT
                id,
                registration_id,
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
                total_claims,
                requires_check_in,
                requires_purchase,
                terms,
                metadata,
                starts_at,
                ends_at,
                published_at,
                created_by,
                updated_by,
                created_at,
                updated_at
            FROM business_promotions
            WHERE registration_id = $1
            ORDER BY starts_at DESC, created_at DESC
            "#,
        )
        .bind(registration_id)
        .fetch_all(&self.pool)
        .await?;

        if promotions.is_empty() {
            return Ok(Vec::new());
        }

        let promotion_ids: Vec<Uuid> = promotions.iter().map(|promotion| promotion.id).collect();
        let mut location_map = self.fetch_locations_for_promotions(&promotion_ids).await?;

        let result = promotions
            .into_iter()
            .map(|promotion| {
                let locations = location_map.remove(&promotion.id).unwrap_or_default();
                BusinessPromotionWithLocations {
                    promotion,
                    locations,
                }
            })
            .collect();

        Ok(result)
    }

    pub async fn list_pending_reviews(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PendingBusinessReview>, sqlx::Error> {
        let records = sqlx::query_as::<_, PendingBusinessReview>(
            r#"
            SELECT
                id,
                name,
                category,
                address,
                tax_id,
                document_urls,
                submitted_at,
                owner_email,
                owner_username
            FROM business_registration_requests
            WHERE status IN ('pending', 'under_review')
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
        registration_id: uuid::Uuid,
        reviewer_id: Option<uuid::Uuid>,
        reviewer_name: Option<String>,
        action: ReviewAction,
        notes: Option<String>,
        rejection_reason: Option<String>,
        new_status: BusinessVerificationStatus,
    ) -> Result<BusinessRegistration, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let notes_ref = notes.as_deref();
        let rejection_ref = rejection_reason.as_deref();
        let reviewer_name_ref = reviewer_name.as_deref();

        {
            let conn = tx.as_mut();
            sqlx::query(
                r#"
                INSERT INTO business_review_events (
                    registration_id,
                    reviewer_id,
                    reviewer_name,
                    action,
                    notes,
                    rejection_reason
                ) VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(registration_id)
            .bind(reviewer_id)
            .bind(reviewer_name_ref)
            .bind(action)
            .bind(notes_ref)
            .bind(rejection_ref)
            .execute(conn)
            .await?;
        }

        let updated = {
            let conn = tx.as_mut();
            sqlx::query_as::<_, BusinessRegistration>(
                r#"
                UPDATE business_registration_requests
                SET
                    status = $2,
                    rejection_reason = $3,
                    reviewer_notes = COALESCE($4, reviewer_notes),
                    reviewer_id = COALESCE($5, reviewer_id),
                    reviewer_name = COALESCE($6, reviewer_name),
                    updated_at = NOW()
                WHERE id = $1
                RETURNING
                    id,
                    user_id,
                    business_id,
                    name,
                    category,
                    address,
                    description,
                    phone,
                    website,
                    tax_id,
                    document_urls,
                    is_multi_user_team,
                    status,
                    owner_email,
                    owner_username,
                    rejection_reason,
                    reviewer_notes,
                    reviewer_id,
                    reviewer_name,
                    submitted_at,
                    updated_at
                "#,
            )
            .bind(registration_id)
            .bind(new_status)
            .bind(rejection_ref)
            .bind(notes_ref)
            .bind(reviewer_id)
            .bind(reviewer_name_ref)
            .fetch_one(conn)
            .await?
        };

        tx.commit().await?;

        Ok(updated)
    }

    pub async fn list_review_events(
        &self,
        registration_id: uuid::Uuid,
    ) -> Result<Vec<BusinessReviewEvent>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessReviewEvent>(
            r#"
            SELECT
                id,
                registration_id,
                reviewer_id,
                reviewer_name,
                action,
                notes,
                rejection_reason,
                created_at
            FROM business_review_events
            WHERE registration_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(registration_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn get_review_stats(&self) -> Result<ReviewStats, sqlx::Error> {
        let record = sqlx::query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status = 'pending') AS pending,
                COUNT(*) FILTER (WHERE status = 'under_review') AS under_review,
                COUNT(*) FILTER (WHERE status = 'approved'
                    AND submitted_at >= NOW() - INTERVAL '1 day') AS approved_today,
                COUNT(*) FILTER (WHERE status = 'rejected'
                    AND submitted_at >= NOW() - INTERVAL '1 day') AS rejected_today
            FROM business_registration_requests
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(ReviewStats {
            pending: record.try_get::<i64, _>("pending")?,
            under_review: record.try_get::<i64, _>("under_review")?,
            approved_today: record.try_get::<i64, _>("approved_today")?,
            rejected_today: record.try_get::<i64, _>("rejected_today")?,
        })
    }

    pub async fn create_company(
        &self,
        owner_user_id: Uuid,
        company_name: String,
        tax_id: Option<String>,
        legal_entity_type: Option<String>,
    ) -> Result<BusinessCompany, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessCompany>(
            r#"
            INSERT INTO business_companies (owner_user_id, company_name, tax_id, legal_entity_type)
            VALUES ($1, $2, $3, $4)
            RETURNING id, owner_user_id, company_name, tax_id, legal_entity_type, is_active, metadata, created_at, updated_at
            "#,
        )
        .bind(owner_user_id)
        .bind(company_name)
        .bind(tax_id)
        .bind(legal_entity_type)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn get_company(&self, company_id: Uuid) -> Result<Option<BusinessCompany>, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessCompany>(
            r#"
            SELECT id, owner_user_id, company_name, tax_id, legal_entity_type, is_active, metadata, created_at, updated_at
            FROM business_companies
            WHERE id = $1
            "#,
        )
        .bind(company_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn list_companies_for_user(&self, owner_user_id: Uuid) -> Result<Vec<BusinessCompany>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessCompany>(
            r#"
            SELECT id, owner_user_id, company_name, tax_id, legal_entity_type, is_active, metadata, created_at, updated_at
            FROM business_companies
            WHERE owner_user_id = $1 AND is_active = TRUE
            ORDER BY created_at DESC
            "#,
        )
        .bind(owner_user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn update_company(&self, company: BusinessCompany) -> Result<BusinessCompany, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessCompany>(
            r#"
            UPDATE business_companies
            SET company_name = $2, tax_id = $3, legal_entity_type = $4, is_active = $5, metadata = $6, updated_at = NOW()
            WHERE id = $1
            RETURNING id, owner_user_id, company_name, tax_id, legal_entity_type, is_active, metadata, created_at, updated_at
            "#,
        )
        .bind(company.id)
        .bind(company.company_name)
        .bind(company.tax_id)
        .bind(company.legal_entity_type)
        .bind(company.is_active)
        .bind(company.metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn delete_company(&self, company_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"DELETE FROM business_companies WHERE id = $1"#)
            .bind(company_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Get an existing approved registration or create a new auto-approved one for managing units
    pub async fn get_or_create_auto_registration(
        &self,
        user_id: Uuid,
        unit_name: &str,
        category: &str,
    ) -> Result<Uuid, sqlx::Error> {
        // First, try to find an existing approved registration for this user
        let existing = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM business_registration_requests
            WHERE user_id = $1 AND status = 'approved'
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(reg_id) = existing {
            return Ok(reg_id);
        }

        // No approved registration exists, create an auto-approved one
        let reg_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO business_registration_requests (
                id, user_id, name, category, address, status, 
                owner_email, owner_username, submitted_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, 'approved', $6, $7, $8, $9)
            "#,
        )
        .bind(reg_id)
        .bind(user_id)
        .bind(unit_name)
        .bind(category)
        .bind("Auto-generated for unit management") // placeholder address
        .bind(format!("user-{user_id}@auto.local")) // placeholder email
        .bind(format!("user-{user_id}")) // placeholder username
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(reg_id)
    }

    pub async fn create_business_unit(
        &self,
        company_id: Uuid,
        registration_id: Option<Uuid>,
        unit_name: String,
        category: String,
        is_primary: bool,
    ) -> Result<BusinessUnit, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        if is_primary {
            sqlx::query(r#"UPDATE business_units SET is_primary = FALSE WHERE company_id = $1"#)
                .bind(company_id)
                .execute(tx.as_mut())
                .await?;
        }

        let record = sqlx::query_as::<_, BusinessUnit>(
            r#"
            INSERT INTO business_units (company_id, registration_id, unit_name, category, is_primary)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, company_id, registration_id, business_id, unit_name, category, is_primary, is_active, metadata, created_at, updated_at
            "#,
        )
        .bind(company_id)
        .bind(registration_id)
        .bind(unit_name)
        .bind(category)
        .bind(is_primary)
        .fetch_one(tx.as_mut())
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    pub async fn get_business_unit(&self, unit_id: Uuid) -> Result<Option<BusinessUnit>, sqlx::Error> {
        let record = sqlx::query_as::<_, BusinessUnit>(
            r#"
            SELECT id, company_id, registration_id, business_id, unit_name, category, is_primary, is_active, metadata, created_at, updated_at
            FROM business_units
            WHERE id = $1
            "#,
        )
        .bind(unit_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn list_units_for_company(&self, company_id: Uuid) -> Result<Vec<BusinessUnit>, sqlx::Error> {
        let records = sqlx::query_as::<_, BusinessUnit>(
            r#"
            SELECT id, company_id, registration_id, business_id, unit_name, category, is_primary, is_active, metadata, created_at, updated_at
            FROM business_units
            WHERE company_id = $1 AND is_active = TRUE
            ORDER BY is_primary DESC, created_at ASC
            "#,
        )
        .bind(company_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    pub async fn update_business_unit(&self, unit: BusinessUnit) -> Result<BusinessUnit, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        if unit.is_primary {
            sqlx::query(r#"UPDATE business_units SET is_primary = FALSE WHERE company_id = $1 AND id != $2"#)
                .bind(unit.company_id)
                .bind(unit.id)
                .execute(tx.as_mut())
                .await?;
        }

        let record = sqlx::query_as::<_, BusinessUnit>(
            r#"
            UPDATE business_units
            SET unit_name = $2, category = $3, is_primary = $4, is_active = $5, metadata = $6, updated_at = NOW()
            WHERE id = $1
            RETURNING id, company_id, registration_id, business_id, unit_name, category, is_primary, is_active, metadata, created_at, updated_at
            "#,
        )
        .bind(unit.id)
        .bind(unit.unit_name)
        .bind(unit.category)
        .bind(unit.is_primary)
        .bind(unit.is_active)
        .bind(unit.metadata)
        .fetch_one(tx.as_mut())
        .await?;

        tx.commit().await?;

        Ok(record)
    }

    pub async fn set_primary_unit(&self, company_id: Uuid, unit_id: Uuid) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(r#"UPDATE business_units SET is_primary = FALSE WHERE company_id = $1"#)
            .bind(company_id)
            .execute(tx.as_mut())
            .await?;

        sqlx::query(r#"UPDATE business_units SET is_primary = TRUE WHERE id = $1"#)
            .bind(unit_id)
            .execute(tx.as_mut())
            .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn delete_business_unit(&self, unit_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(r#"DELETE FROM business_units WHERE id = $1"#)
            .bind(unit_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_company_with_units(&self, company_id: Uuid) -> Result<Option<CompanyWithUnits>, sqlx::Error> {
        let company = match self.get_company(company_id).await? {
            Some(c) => c,
            None => return Ok(None),
        };

        let units = self.list_units_for_company(company_id).await?;

        let mut unit_details = Vec::new();
        for unit in units {
            let registration = if let Some(reg_id) = unit.registration_id {
                self.get_registration_by_id(reg_id).await?
            } else {
                None
            };

            let locations = if let Some(reg_id) = unit.registration_id {
                self.list_locations_for_registration(reg_id).await?
            } else {
                Vec::new()
            };

            let promotions = if let Some(reg_id) = unit.registration_id {
                self.list_promotions_for_registration(reg_id).await?
            } else {
                Vec::new()
            };

            unit_details.push(BusinessUnitDetail {
                unit,
                registration,
                locations,
                promotions,
            });
        }

        Ok(Some(CompanyWithUnits {
            company,
            units: unit_details,
        }))
    }

    pub async fn get_unit_detail(&self, unit_id: Uuid) -> Result<Option<BusinessUnitDetail>, sqlx::Error> {
        let unit = match self.get_business_unit(unit_id).await? {
            Some(u) => u,
            None => return Ok(None),
        };

        let registration = if let Some(reg_id) = unit.registration_id {
            self.get_registration_by_id(reg_id).await?
        } else {
            None
        };

        let locations = if let Some(reg_id) = unit.registration_id {
            self.list_locations_for_registration(reg_id).await?
        } else {
            Vec::new()
        };

        let promotions = if let Some(reg_id) = unit.registration_id {
            self.list_promotions_for_registration(reg_id).await?
        } else {
            Vec::new()
        };

        Ok(Some(BusinessUnitDetail {
            unit,
            registration,
            locations,
            promotions,
        }))
    }
}

async fn create_database_if_missing(database_url: &str) -> Result<(), sqlx::Error> {
    let options: PgConnectOptions = database_url.parse()?;
    let database_name = options
        .get_database()
        .map(|name| name.to_string())
        .unwrap_or_else(|| "postgres".to_string());

    // If we're already targeting the default maintenance database, nothing to do.
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
