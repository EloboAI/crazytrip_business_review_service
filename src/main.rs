mod database;
mod handlers;
mod models;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use std::env;

use crate::database::Database;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8082".to_string());
    let bind_address = format!("{}:{}", host, port);

    let database_url = env::var("DATABASE_URL").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "DATABASE_URL must be set in environment",
        )
    })?;

    let db = Database::connect(&database_url).await.map_err(|err| {
        log::error!("Failed to initialize database: {err:?}");
        std::io::Error::new(std::io::ErrorKind::Other, err)
    })?;

    let db_data = web::Data::new(db);

    log::info!(
        "🚀 Starting CrazyTrip Business Review Service on {}",
        bind_address
    );

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(db_data.clone())
            .wrap(cors)
            .wrap(Logger::default())
            .service(
                web::scope("/api/v1")
                    .service(handlers::health_check)
                    .service(handlers::submit_registration)
                    .service(handlers::get_registration)
                    .service(handlers::get_latest_registration_for_user)
                    .service(handlers::list_registrations_for_user)
                    .service(handlers::create_location_for_registration)
                    .service(handlers::update_location_for_registration)
                    .service(handlers::delete_location_for_registration)
                    .service(handlers::list_promotions_for_registration)
                    .service(handlers::get_promotion_for_registration)
                    .service(handlers::create_promotion_for_registration)
                    .service(handlers::update_promotion_for_registration)
                    .service(handlers::delete_promotion_for_registration)
                    // Company management
                    .service(handlers::create_company)
                    .service(handlers::get_company)
                    .service(handlers::list_companies)
                    .service(handlers::update_company)
                    .service(handlers::delete_company)
                    .service(handlers::get_company_with_units)
                    // Business unit management
                    .service(handlers::create_business_unit)
                    .service(handlers::get_business_unit)
                    .service(handlers::list_business_units)
                    .service(handlers::update_business_unit)
                    .service(handlers::set_primary_unit)
                    .service(handlers::delete_business_unit)
                    // Review system
                    .service(handlers::list_pending_reviews)
                    .service(handlers::get_business_review)
                    .service(handlers::submit_review_action)
                    .service(handlers::get_review_stats),
            )
    })
    .bind(&bind_address)?
    .run()
    .await
}
