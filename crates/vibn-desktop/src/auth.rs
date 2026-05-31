use keyring::Entry;
use serde::{Deserialize, Serialize};
use vibn_core::{load_config, save_config};

const KEYCHAIN_SERVICE: &str = "com.vibn.desktop";
const SESSION_TOKEN_ACCOUNT: &str = "better-auth-session";
const DEFAULT_AUTH_ENDPOINT: &str = "https://vibn-auth-api.vercel.app";

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct UserProfile {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub auth_endpoint: String,
    #[serde(default)]
    pub signed_in: bool,
}

fn read_profile() -> UserProfile {
    let cfg = match load_config() {
        Ok(c) => c,
        Err(_) => return UserProfile::default(),
    };
    cfg.extra
        .get("user_profile")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn write_profile(profile: &UserProfile) -> Result<(), String> {
    let mut cfg = load_config().map_err(|e| e.to_string())?;
    cfg.extra.insert(
        "user_profile".to_owned(),
        serde_json::to_value(profile).map_err(|e| e.to_string())?,
    );
    save_config(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_user_profile() -> UserProfile {
    read_profile()
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_user_profile(profile: UserProfile) -> Result<UserProfile, String> {
    write_profile(&profile)?;
    Ok(profile)
}

fn keychain_entry(account: &str) -> Result<Entry, String> {
    Entry::new(KEYCHAIN_SERVICE, account).map_err(|e| format!("keychain: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_credential(account: String, password: String) -> Result<(), String> {
    let entry = keychain_entry(&account)?;
    entry
        .set_password(&password)
        .map_err(|e| format!("keychain set: {e}"))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_credential(account: String) -> Result<Option<String>, String> {
    let entry = keychain_entry(&account)?;
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("keychain get: {e}")),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_credential(account: String) -> Result<(), String> {
    let entry = keychain_entry(&account)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keychain delete: {e}")),
    }
}

#[derive(Debug, Deserialize)]
pub struct SignInInput {
    pub email: String,
    pub password: String,
    pub remember: bool,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SignUpInput {
    pub email: String,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub remember: bool,
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SignInOutput {
    pub profile: UserProfile,
    pub token_preview: Option<String>,
    pub note: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn sign_in(input: SignInInput) -> Result<SignInOutput, String> {
    let mut profile = read_profile();
    profile.email = input.email.clone();
    if profile.display_name.is_empty() {
        profile.display_name = input
            .email
            .split('@')
            .next()
            .unwrap_or(&input.email)
            .to_owned();
    }

    let endpoint = input
        .endpoint
        .as_ref()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if profile.auth_endpoint.is_empty() {
                None
            } else {
                Some(profile.auth_endpoint.clone())
            }
        })
        .or_else(|| std::env::var("VIBN_AUTH_ENDPOINT").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| DEFAULT_AUTH_ENDPOINT.to_owned());

    profile.auth_endpoint = endpoint.clone();

    let mut token_preview: Option<String> = None;
    let note;

    {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;
        let url = format!("{}/api/auth/sign-in/email", endpoint.trim_end_matches('/'));
        let resp = client
            .post(&url)
            .json(&serde_json::json!({
                "email": input.email,
                "password": input.password,
                "rememberMe": input.remember,
            }))
            .send()
            .map_err(|e| format!("sign-in request: {e}"))?;
        let status = resp.status();
        let header_token = resp
            .headers()
            .get("set-auth-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body_text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("sign-in failed ({status}): {body_text}"));
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&body_text).unwrap_or(serde_json::Value::Null);
        let body_token = parsed
            .get("token")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let display_name = parsed
            .get("user")
            .and_then(|u| u.get("name"))
            .and_then(|n| n.as_str())
            .map(str::to_owned);
        if let Some(name) = display_name {
            if !name.is_empty() {
                profile.display_name = name;
            }
        }
        let token = header_token.or(body_token);
        if let Some(t) = token.clone() {
            token_preview = Some(format!("{}…", &t[..t.len().min(8)]));
            keychain_entry(SESSION_TOKEN_ACCOUNT)?
                .set_password(&t)
                .map_err(|e| format!("token store: {e}"))?;
        }
        profile.signed_in = true;
        note = if token.is_some() {
            "Signed in.".to_owned()
        } else {
            "Signed in (no token returned).".to_owned()
        };
    }

    if input.remember {
        keychain_entry(&format!("password:{}", input.email))?
            .set_password(&input.password)
            .map_err(|e| format!("password store: {e}"))?;
    }
    write_profile(&profile)?;

    Ok(SignInOutput {
        profile,
        token_preview,
        note,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn sign_up(input: SignUpInput) -> Result<SignInOutput, String> {
    let mut profile = read_profile();
    let full_name = format!("{} {}", input.first_name, input.last_name)
        .trim()
        .to_owned();
    profile.email = input.email.clone();
    profile.display_name = if full_name.is_empty() {
        input
            .email
            .split('@')
            .next()
            .unwrap_or(&input.email)
            .to_owned()
    } else {
        full_name.clone()
    };

    let endpoint = input
        .endpoint
        .as_ref()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if profile.auth_endpoint.is_empty() {
                None
            } else {
                Some(profile.auth_endpoint.clone())
            }
        })
        .or_else(|| std::env::var("VIBN_AUTH_ENDPOINT").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| DEFAULT_AUTH_ENDPOINT.to_owned());
    profile.auth_endpoint = endpoint.clone();

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{}/api/auth/sign-up/email", endpoint.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "email": input.email,
            "password": input.password,
            "name": if full_name.is_empty() { input.email.clone() } else { full_name.clone() },
            "firstName": input.first_name,
            "lastName": input.last_name,
        }))
        .send()
        .map_err(|e| format!("sign-up request: {e}"))?;
    let status = resp.status();
    let body_text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("sign-up failed ({status}): {body_text}"));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&body_text).unwrap_or(serde_json::Value::Null);
    let body_token = parsed
        .get("token")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let mut token_preview = None;
    if let Some(t) = body_token.clone() {
        token_preview = Some(format!("{}…", &t[..t.len().min(8)]));
        keychain_entry(SESSION_TOKEN_ACCOUNT)?
            .set_password(&t)
            .map_err(|e| format!("token store: {e}"))?;
    }
    profile.signed_in = true;

    if input.remember {
        keychain_entry(&format!("password:{}", input.email))?
            .set_password(&input.password)
            .map_err(|e| format!("password store: {e}"))?;
    }
    write_profile(&profile)?;

    Ok(SignInOutput {
        profile,
        token_preview,
        note: "Account created.".to_owned(),
    })
}

#[tauri::command]
pub fn sign_out() -> Result<UserProfile, String> {
    if let Ok(entry) = keychain_entry(SESSION_TOKEN_ACCOUNT) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(format!("token delete: {e}")),
        }
    }
    let mut profile = read_profile();
    profile.signed_in = false;
    write_profile(&profile)?;
    Ok(profile)
}
