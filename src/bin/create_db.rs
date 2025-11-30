use tokio_postgres::NoTls;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conn_str = std::env::var("PG_ADMIN_CONN").unwrap_or_else(|_| "host=127.0.0.1 user=postgres password=moti dbname=postgres".into());

    println!("Connecting to Postgres to manage databases...");

    let (client, connection) = tokio_postgres::connect(&conn_str, NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "crazytrip_reviews".into());

    let row = client
        .query_opt("SELECT 1 FROM pg_database WHERE datname = $1", &[&db_name])
        .await?;

    if row.is_some() {
        println!("Database '{}' already exists.", db_name);
        return Ok(());
    }

    let valid_name = db_name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');

    if !valid_name {
        eprintln!("Refusing to create database: invalid database name '{}'.", db_name);
        return Ok(());
    }

    let create_sql = format!("CREATE DATABASE \"{}\"", db_name);
    match client.execute(create_sql.as_str(), &[]).await {
        Ok(_) => println!("Database '{}' created successfully.", db_name),
        Err(e) => eprintln!("Failed to create database '{}': {}", db_name, e),
    }

    Ok(())
}
