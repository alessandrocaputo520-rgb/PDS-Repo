//! Credenziali conservate con Argon2id e salt casuale.
use crate::storage::Storage;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;

pub fn validate(username: &str, password: &str) -> Result<(), String> {
    if !(3..=32).contains(&username.len())
        || !username
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
    {
        return Err("Username: 3-32 caratteri ASCII (lettere, numeri, _, -, .)".into());
    }
    if !(8..=128).contains(&password.len()) {
        return Err("Password: 8-128 caratteri".into());
    }
    Ok(())
}

pub fn register(
    db: &Storage,
    username: &str,
    password: &str,
    now: i64,
) -> Result<(i64, String), String> {
    validate(username, password)?;
    if db.find_user(username).map_err(|e| e.to_string())?.is_some() {
        return Err("Username già registrato".into());
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| e.to_string())?
        .to_string();
    let id = db
        .add_user(username, &hash, now)
        .map_err(|e| e.to_string())?;
    Ok((id, username.to_owned()))
}

pub fn login(db: &Storage, username: &str, password: &str) -> Result<(i64, String), String> {
    let user = db
        .find_user(username)
        .map_err(|e| e.to_string())?
        .ok_or("Credenziali non valide")?;
    let parsed = PasswordHash::new(&user.password_hash).map_err(|e| e.to_string())?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| "Credenziali non valide".to_string())?;
    Ok((user.id, user.username))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_login_and_bad_password() {
        let db = Storage::open(":memory:").unwrap();
        register(&db, "alice", "password-segreta", 123).unwrap();
        assert!(login(&db, "Alice", "password-segreta").is_ok());
        assert!(login(&db, "alice", "sbagliata").is_err());
        assert!(register(&db, "alice", "password-segreta", 124).is_err());
    }
}
