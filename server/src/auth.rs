use common::models::User;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::storage;

/// Registra un nuovo utente. Restituisce l'ID o un messaggio di errore.
pub fn register(
    db: &Arc<Mutex<Connection>>,
    username: &str,
    password: &str,
) -> Result<i64, String> {
    // Validazione base
    let username = username.trim();
    let password = password.trim();

    if username.len() < 3 || username.len() > 32 {
        return Err("Username deve essere tra 3 e 32 caratteri".to_string());
    }
    if username.chars().any(|c| !c.is_alphanumeric() && c != '_') {
        return Err("Username può contenere solo lettere, cifre e '_'".to_string());
    }
    if password.len() < 6 {
        return Err("Password deve essere almeno 6 caratteri".to_string());
    }

    // Hash bcrypt (cost 10 di default)
    let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("Errore hashing password: {e}"))?;

    let conn = db.lock().unwrap();
    storage::insert_user(&conn, username, &hash).map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "Username già registrato".to_string()
        } else {
            format!("Errore database: {e}")
        }
    })
}

/// Verifica le credenziali e restituisce l'utente o un messaggio di errore.
pub fn login(
    db: &Arc<Mutex<Connection>>,
    username: &str,
    password: &str,
) -> Result<User, String> {
    let username = username.trim();
    let password = password.trim();

    let conn = db.lock().unwrap();
    let user = storage::find_user_by_username(&conn, username)
        .map_err(|e| format!("Errore database: {e}"))?
        .ok_or_else(|| "Username o password errati".to_string())?;

    let valid = bcrypt::verify(password, &user.password_hash)
        .map_err(|e| format!("Errore verifica password: {e}"))?;

    if valid {
        Ok(user)
    } else {
        Err("Username o password errati".to_string())
    }
}
