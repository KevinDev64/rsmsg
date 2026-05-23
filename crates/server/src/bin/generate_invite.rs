use argon2::{
    Argon2,
    password_hash::{
        PasswordHasher, SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use server::repository::registration_invites;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL")?;
    let db = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let id = Uuid::new_v4();
    let secret = generate_secret();
    let secret_hash = hash_secret(&secret)?;
    registration_invites::insert_invite(&db, id, secret_hash).await?;
    println!("Invite code: RSMSG:{id}:{secret}");
    println!("Expires in: 2 days");
    Ok(())
}

fn generate_secret() -> String {
    let mut bytes = [0_u8; 32];
    let mut rng = OsRng;
    rng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_secret(secret: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map_err(|_| anyhow::anyhow!("invite hashing failed"))?;
    Ok(hash.to_string())
}
