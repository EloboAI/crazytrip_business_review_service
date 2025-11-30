mod clients;
mod database;
mod handlers;
mod models;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use std::env;
use std::sync::Arc;

use crate::clients::stories::StoriesClient;
use crate::database::DatabaseService;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables
    dotenv::dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8082".to_string());
    let bind_address = format!("{}:{}", host, port);
    let stories_service_url = env::var("STORIES_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:8083".to_string());

    let database_url = env::var("DATABASE_URL").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "DATABASE_URL must be set in environment",
        )
    })?;

    let db = DatabaseService::new(&database_url).await.map_err(|err| {
        log::error!("Failed to initialize database: {err:?}");
        std::io::Error::new(std::io::ErrorKind::Other, err)
    })?;

    // Initialize schema (though we use migrations, this ensures connection)
    if let Err(e) = db.init_schema().await {
        log::error!("Failed to initialize DB schema: {:#?}", e);
    } else {
        log::info!("DB schema ensured");
    }

    let db_data = web::Data::new(Arc::new(db));
    let stories_client = web::Data::new(StoriesClient::new(stories_service_url));

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
            .app_data(stories_client.clone())
            .wrap(cors)
            .wrap(Logger::default())
            .service(
                web::scope("/api/v1")
                    // Health
                    .service(handlers::health_check)
                    // Registrations (verification workflow)
                    .service(handlers::submit_registration)
                    .service(handlers::get_registration)
                    .service(handlers::get_latest_registration_for_user)
                    .service(handlers::list_registrations_for_user)
                    // Review system
                    .service(handlers::list_pending_reviews)
                    .service(handlers::get_business_review)
                    .service(handlers::submit_review_action)
                    .service(handlers::get_review_stats)
                    // Businesses
                    .service(handlers::create_business)
                    .service(handlers::get_business)
                    .service(handlers::list_businesses_for_user)
                    .service(handlers::update_business)
                    .service(handlers::delete_business)
                    // Locations
                    .service(handlers::create_location)
                    .service(handlers::get_location)
                    .service(handlers::list_locations_for_business)
                    .service(handlers::update_location)
                    .service(handlers::delete_location)
                    // Promotions
                    .service(handlers::create_promotion)
                    .service(handlers::get_promotion)
                    .service(handlers::list_promotions_for_location)
                    .service(handlers::list_promotions_for_business)
                    .service(handlers::update_promotion)
                    .service(handlers::delete_promotion)
                    // Location Admins
                    .service(handlers::add_location_admin)
                    .service(handlers::list_location_admins)
                    .service(handlers::remove_location_admin),
            )
    })
    .bind(&bind_address)?
    .run()
    .await
}
