use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Extension,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use rand::Rng;
use std::{collections::HashMap, env, net::SocketAddr, sync::LazyLock};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use webauthn_rs::prelude::*;

use crate::{
    admin_tasks,
    app::state::SharedAppState,
    app::{
        render::{inject_runtime_tokens, wrap_html_with_options, HtmlOptions, CLIENT_IP_TOKEN},
        state,
    },
    asset::asset_url,
};

type HmacSha256 = Hmac<sha2::Sha256>;
type RegStateEntry = (PasskeyRegistration, bool);
type AuthStateEntry = (PasskeyAuthentication, bool);

const ADMIN_PASSKEY_ENV: &str = "RODIN_ADMIN_PASSKEY";
const ADMIN_SESSION_SECRET_ENV: &str = "RODIN_ADMIN_SESSION_SECRET";
const ADMIN_COOKIE: &str = "rodin-admin-session";
const ADMIN_SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 3; // 3 days

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct StoredPasskey {
    passkey: Passkey,
    rp_id: String,
}

struct AdminAuth {
    wa_prod: Webauthn,
    wa_local: Webauthn,
    passkey: RwLock<Option<StoredPasskey>>,
    reg_states: Mutex<HashMap<String, RegStateEntry>>,
    auth_states: Mutex<HashMap<String, AuthStateEntry>>,
    session_secret: [u8; 32],
}

static ADMIN_AUTH: LazyLock<AdminAuth> = LazyLock::new(|| {
    build_admin_auth().unwrap_or_else(|e| {
        panic!("failed to initialize admin auth: {e:?}");
    })
});

fn build_admin_auth() -> anyhow::Result<AdminAuth> {
    let prod_origin = url::Url::parse(crate::constants::SITE_URL)?;
    let local_origin = url::Url::parse("http://localhost:3000")?;

    let wa_prod = WebauthnBuilder::new(
        crate::constants::SITE_URL.trim_start_matches("https://"),
        &prod_origin,
    )?
    .rp_name("rodin-admin")
    .build()?;
    let wa_local = WebauthnBuilder::new("localhost", &local_origin)?
        .rp_name("rodin-admin-local")
        .build()?;

    let passkey = load_env_passkey();
    let session_secret = load_session_secret();

    Ok(AdminAuth {
        wa_prod,
        wa_local,
        passkey: RwLock::new(passkey),
        reg_states: Mutex::new(HashMap::new()),
        auth_states: Mutex::new(HashMap::new()),
        session_secret,
    })
}

fn load_session_secret() -> [u8; 32] {
    if let Ok(val) = env::var(ADMIN_SESSION_SECRET_ENV) {
        let mut buf = [0u8; 32];
        if let Ok(decoded) = URL_SAFE_NO_PAD.decode(val.as_bytes()) {
            for (i, b) in decoded.iter().take(32).enumerate() {
                buf[i] = *b;
            }
            if decoded.len() >= 32 {
                return buf;
            }
        }
    }
    let mut rng = rand::rng();
    let mut bytes = [0u8; 32];
    for b in bytes.iter_mut() {
        *b = rng.random();
    }
    bytes
}

fn load_env_passkey() -> Option<StoredPasskey> {
    let raw = env::var(ADMIN_PASSKEY_ENV).ok()?;
    let decoded = URL_SAFE_NO_PAD.decode(raw.as_bytes()).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn encode_env_passkey(passkey: &StoredPasskey) -> String {
    let json = serde_json::to_vec(passkey).unwrap_or_default();
    URL_SAFE_NO_PAD.encode(json)
}

fn verify_session(headers: &HeaderMap) -> bool {
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let mut session_val: Option<&str> = None;
    for pair in cookie_header.split(';') {
        let trimmed = pair.trim();
        if let Some(val) = trimmed.strip_prefix(&format!("{}=", ADMIN_COOKIE)) {
            session_val = Some(val);
            break;
        }
    }
    let val = match session_val {
        Some(v) => v,
        None => return false,
    };
    let mut parts = val.split('.');
    let payload_b64 = match parts.next() {
        Some(p) => p,
        None => return false,
    };
    let sig_b64 = match parts.next() {
        Some(s) => s,
        None => return false,
    };
    if parts.next().is_some() {
        return false;
    }
    let payload = match URL_SAFE_NO_PAD.decode(payload_b64.as_bytes()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let sig = match URL_SAFE_NO_PAD.decode(sig_b64.as_bytes()) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let mut mac = HmacSha256::new_from_slice(&ADMIN_AUTH.session_secret).unwrap();
    mac.update(&payload);
    if mac.verify_slice(&sig).is_err() {
        return false;
    }

    if payload.len() < 8 {
        return false;
    }
    let mut exp_bytes = [0u8; 8];
    exp_bytes.copy_from_slice(&payload[..8]);
    let exp = i64::from_be_bytes(exp_bytes);
    let now = chrono::Utc::now().timestamp();
    now <= exp
}

fn issue_session_cookie(host: &str) -> String {
    let now = chrono::Utc::now().timestamp();
    let exp = now + ADMIN_SESSION_TTL_SECS;
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(&exp.to_be_bytes());
    let mut rng = rand::rng();
    for _ in 0..8 {
        payload.push(rng.random());
    }
    let mut mac = HmacSha256::new_from_slice(&ADMIN_AUTH.session_secret).unwrap();
    mac.update(&payload);
    let sig = mac.finalize().into_bytes();
    let val = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(payload),
        URL_SAFE_NO_PAD.encode(sig)
    );
    let secure = if host == "localhost" || host == "127.0.0.1" {
        ""
    } else {
        "; Secure"
    };
    format!(
        "{}={}; Max-Age={}; Path=/__admin; HttpOnly; SameSite=Strict{}",
        ADMIN_COOKIE, val, ADMIN_SESSION_TTL_SECS, secure
    )
}

fn select_webauthn<'a>(auth: &'a AdminAuth, host: &str) -> (&'a Webauthn, bool) {
    if host == "localhost" || host == "127.0.0.1" {
        (&auth.wa_local, true)
    } else {
        (&auth.wa_prod, false)
    }
}

fn admin_page_html(_client_ip: &str, nonce: &str) -> String {
    let css = asset_url("/assets/build/admin.css");
    let js = asset_url("/assets/build/admin.js");
    let mut meta = HashMap::new();
    meta.insert("robots".to_string(), "noindex, nofollow".to_string());
    let opts = HtmlOptions {
        meta: Some(meta),
        head_links: vec![format!(r#"<link rel="stylesheet" href="{css}" />"#)],
        head_scripts: vec![format!(
            r#"<script nonce="{nonce}" src="{js}" defer></script>"#
        )],
        ..Default::default()
    };

    let body = format!(
        r#"<div class="admin-shell">
  <header class="admin-header">
    <div>
      <h1>Rodin Admin</h1>
      <p class="muted">client: {CLIENT_IP_TOKEN}</p>
    </div>
    <div class="chip" id="auth-status">checking…</div>
  </header>

  <section id="login-panel" class="card">
    <h2>ログイン</h2>
    <p class="muted">パスキーでサインインします。</p>
    <button id="login-btn">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
        パスキーでログイン
    </button>
    <div class="small muted" id="login-hint"></div>
  </section>

  <section id="register-panel" class="card hidden">
    <h2>初回セットアップ（登録）</h2>
    <p class="muted">まだサーバーにパスキーが登録されていない場合に一度だけ実行します。完了後に表示される値を環境変数 <code>{ADMIN_PASSKEY_ENV}</code> にセットし、サービスを再起動してください。</p>
    <button id="register-btn">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 1 1-7.778 7.778 5.5 5.5 0 0 1 7.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4"/></svg>
        パスキーを登録
    </button>
    <pre id="register-output" class="mono"></pre>
  </section>

  <section id="actions" class="card hidden">
    <h2>メンテナンス</h2>
    <div class="actions-grid">
      <div>
        <h3>ビルド &amp; OpenSearch</h3>
        <p class="muted">Typst→HTML生成、Markdown生成、サイトマップ更新、OpenSearch への再投入をまとめて実行します。</p>
        <label><input type="checkbox" id="reset-os" /> インデックスを削除してから再作成</label><br />
        <label><input type="checkbox" id="skip-md" /> Markdown 生成をスキップ</label>
        <button id="run-build">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg>
            実行（リビルド &amp; インデックス）
        </button>
      </div>
      <div>
        <h3>ライブリロード</h3>
        <p class="muted">静的生成済みの内容をアプリに再読み込みします。</p>
        <button id="reload-btn">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M23 4v6h-6"/><path d="M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
            状態をリロード
        </button>
      </div>
      <div>
        <h3>Git Pull</h3>
        <p class="muted">contentリポジトリを更新します。</p>
        <button id="git-pull-btn">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M13 6h3a2 2 0 0 1 2 2v7"/><path d="M6 9v12"/></svg>
            Git Pull
        </button>
      </div>
      <div>
        <h3>フォントサブセット</h3>
        <p class="muted">contentとソースファイルからグリフを収集してフォントを最適化します。</p>
        <button id="font-subset-btn">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 7 4 4 20 4 20 7"/><line x1="9" y1="20" x2="15" y2="20"/><line x1="12" y1="4" x2="12" y2="20"/></svg>
            フォントサブセット実行
        </button>
      </div>
      <div>
        <h3>Cloudflare キャッシュパージ</h3>
        <p class="muted">CloudflareのCDNキャッシュをクリアします。</p>
        <button id="purge-cache-btn">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6L6 18"/><path d="M6 6l12 12"/></svg>
            キャッシュパージ
        </button>
      </div>
    </div>
    <pre id="log" class="mono"></pre>
  </section>
</div>"#
    );

    wrap_html_with_options(&body, "Admin", &opts)
}

#[derive(serde::Serialize)]
pub struct AdminStatus {
    pub logged_in: bool,
    pub has_credential: bool,
}

pub async fn admin_page_handler(
    State(_state): State<SharedAppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Extension(nonce): Extension<String>,
) -> Response {
    let client_ip = super::get_client_ip(&headers, &addr);
    let html = inject_runtime_tokens(&admin_page_html(&client_ip, &nonce), &client_ip, &nonce);
    let mut res = Html(html).into_response();
    res.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    res
}

pub async fn admin_status_handler(headers: HeaderMap) -> impl IntoResponse {
    let logged_in = verify_session(&headers);
    let has_cred = ADMIN_AUTH.passkey.read().await.is_some();
    (
        StatusCode::OK,
        axum::Json(AdminStatus {
            logged_in,
            has_credential: has_cred,
        }),
    )
}

#[derive(serde::Serialize)]
struct RegisterOptionsResponse {
    options: CreationChallengeResponse,
    challenge_b64: String,
}

pub async fn admin_register_options_handler(
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    if ADMIN_AUTH.passkey.read().await.is_some() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(crate::constants::SITE_URL.trim_start_matches("https://"))
        .split(':')
        .next()
        .unwrap_or(crate::constants::SITE_URL.trim_start_matches("https://"));
    let (wa, is_local) = select_webauthn(&ADMIN_AUTH, host);
    let user_id = Uuid::new_v4();
    let (options, state) = wa
        .start_passkey_registration(user_id, "rodin-admin", "Rodin Admin", None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let challenge = options.public_key.challenge.clone();
    {
        let challenge_b64 = URL_SAFE_NO_PAD.encode(challenge.as_ref());
        let mut guard = ADMIN_AUTH.reg_states.lock().await;
        guard.insert(challenge_b64.clone(), (state, is_local));
    }
    let challenge_b64 = URL_SAFE_NO_PAD.encode(challenge.as_ref());
    Ok(axum::Json(RegisterOptionsResponse {
        options,
        challenge_b64,
    }))
}

#[derive(serde::Deserialize)]
pub struct FinishRegisterReq {
    credential: RegisterPublicKeyCredential,
    challenge: String,
}

#[derive(serde::Serialize)]
pub struct FinishRegisterResp {
    env_value: String,
}

pub async fn admin_register_finish_handler(
    axum::Json(payload): axum::Json<FinishRegisterReq>,
) -> Response {
    if ADMIN_AUTH.passkey.read().await.is_some() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let (state, is_local) = {
        let mut guard = ADMIN_AUTH.reg_states.lock().await;
        match guard.remove(&payload.challenge) {
            Some(v) => v,
            None => return StatusCode::BAD_REQUEST.into_response(),
        }
    };

    let passkey = match if is_local {
        &ADMIN_AUTH.wa_local
    } else {
        &ADMIN_AUTH.wa_prod
    }
    .finish_passkey_registration(&payload.credential, &state)
    {
        Ok(p) => p,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let stored = StoredPasskey {
        passkey: passkey.clone(),
        rp_id: if is_local {
            "localhost".to_string()
        } else {
            crate::constants::SITE_URL
                .trim_start_matches("https://")
                .to_string()
        },
    };
    {
        let mut guard = ADMIN_AUTH.passkey.write().await;
        *guard = Some(stored.clone());
    }
    let env_value = encode_env_passkey(&stored);
    axum::Json(FinishRegisterResp { env_value }).into_response()
}

#[derive(serde::Serialize)]
struct LoginOptionsResp {
    options: RequestChallengeResponse,
    challenge_b64: String,
}

pub async fn admin_login_options_handler(
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let passkey = match ADMIN_AUTH.passkey.read().await.clone() {
        Some(p) => p,
        None => return Err(StatusCode::BAD_REQUEST),
    };
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("suzuneu.com")
        .split(':')
        .next()
        .unwrap_or("suzuneu.com");
    let (wa, is_local) = select_webauthn(&ADMIN_AUTH, host);
    let allow = vec![passkey.passkey.clone()];
    let (options, state) = wa
        .start_passkey_authentication(&allow)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let challenge = options.public_key.challenge.clone();
    {
        let challenge_b64 = URL_SAFE_NO_PAD.encode(challenge.as_ref());
        let mut guard = ADMIN_AUTH.auth_states.lock().await;
        guard.insert(challenge_b64.clone(), (state, is_local));
    }
    let challenge_b64 = URL_SAFE_NO_PAD.encode(challenge.as_ref());
    Ok(axum::Json(LoginOptionsResp {
        options,
        challenge_b64,
    }))
}

#[derive(serde::Deserialize)]
pub struct FinishLoginReq {
    credential: PublicKeyCredential,
    challenge: String,
}

pub async fn admin_login_finish_handler(
    headers: HeaderMap,
    axum::Json(payload): axum::Json<FinishLoginReq>,
) -> Response {
    let stored = match ADMIN_AUTH.passkey.read().await.clone() {
        Some(p) => p,
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    let (state, is_local) = {
        let mut guard = ADMIN_AUTH.auth_states.lock().await;
        match guard.remove(&payload.challenge) {
            Some(v) => v,
            None => return StatusCode::BAD_REQUEST.into_response(),
        }
    };

    let wa = if is_local {
        &ADMIN_AUTH.wa_local
    } else {
        &ADMIN_AUTH.wa_prod
    };

    let res = match wa.finish_passkey_authentication(&payload.credential, &state) {
        Ok(r) => r,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };

    if res.needs_update() {
        let mut pass = stored.passkey.clone();
        pass.update_credential(&res);
        let mut guard = ADMIN_AUTH.passkey.write().await;
        *guard = Some(StoredPasskey {
            passkey: pass,
            rp_id: stored.rp_id.clone(),
        });
    }

    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("suzuneu.com")
        .split(':')
        .next()
        .unwrap_or("suzuneu.com");
    let cookie = issue_session_cookie(host);
    let mut res = Response::new(axum::body::Body::empty());
    *res.status_mut() = StatusCode::OK;
    res.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_str(&cookie).unwrap(),
    );
    res
}

#[derive(serde::Deserialize)]
pub struct AdminRunReq {
    opensearch: bool,
    reset_os: bool,
    skip_markdown: bool,
}

#[derive(serde::Serialize)]
struct AdminRunResp {
    log: String,
    success: bool,
    error: Option<String>,
}

pub async fn admin_run_handler(
    State(shared): State<SharedAppState>,
    headers: HeaderMap,
    axum::Json(payload): axum::Json<AdminRunReq>,
) -> Result<impl IntoResponse, StatusCode> {
    if !verify_session(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let res = match tokio::task::spawn_blocking(move || {
        admin_tasks::run_build_and_index(
            payload.opensearch,
            payload.reset_os,
            payload.skip_markdown,
        )
    })
    .await
    {
        Ok(Ok(log)) => AdminRunResp {
            log,
            success: true,
            error: None,
        },
        Ok(Err(e)) => AdminRunResp {
            log: String::new(),
            success: false,
            error: Some(format!("{e:#}")),
        },
        Err(e) => AdminRunResp {
            log: String::new(),
            success: false,
            error: Some(format!("Task panicked: {e}")),
        },
    };

    if res.success {
        if state::reload_state(&shared).await.is_err() {
            return Ok(axum::Json(AdminRunResp {
                log: res.log,
                success: false,
                error: Some("Failed to reload state".to_string()),
            }));
        }
    }

    Ok(axum::Json(res))
}

#[derive(serde::Serialize)]
struct AdminReloadResp {
    message: String,
}

pub async fn admin_reload_handler(
    State(shared): State<SharedAppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    if !verify_session(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    match state::reload_state(&shared).await {
        Ok(()) => Ok(axum::Json(AdminReloadResp {
            message: "reloaded".to_string(),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(serde::Serialize)]
struct AdminTaskResp {
    log: String,
    success: bool,
    error: Option<String>,
}

pub async fn admin_git_pull_handler(headers: HeaderMap) -> Result<impl IntoResponse, StatusCode> {
    if !verify_session(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let res = match tokio::task::spawn_blocking(admin_tasks::run_git_pull).await {
        Ok(Ok(log)) => AdminTaskResp {
            log,
            success: true,
            error: None,
        },
        Ok(Err(e)) => AdminTaskResp {
            log: String::new(),
            success: false,
            error: Some(format!("{e:#}")),
        },
        Err(e) => AdminTaskResp {
            log: String::new(),
            success: false,
            error: Some(format!("Task panicked: {e}")),
        },
    };
    Ok(axum::Json(res))
}

pub async fn admin_font_subset_handler(
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    if !verify_session(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let res = match tokio::task::spawn_blocking(admin_tasks::run_font_subset).await {
        Ok(Ok(log)) => AdminTaskResp {
            log,
            success: true,
            error: None,
        },
        Ok(Err(e)) => AdminTaskResp {
            log: String::new(),
            success: false,
            error: Some(format!("{e:#}")),
        },
        Err(e) => AdminTaskResp {
            log: String::new(),
            success: false,
            error: Some(format!("Task panicked: {e}")),
        },
    };
    Ok(axum::Json(res))
}

pub async fn admin_purge_cache_handler(
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    if !verify_session(&headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let res = match tokio::task::spawn_blocking(admin_tasks::purge_cloudflare_cache).await {
        Ok(Ok(log)) => AdminTaskResp {
            log,
            success: true,
            error: None,
        },
        Ok(Err(e)) => AdminTaskResp {
            log: String::new(),
            success: false,
            error: Some(format!("{e:#}")),
        },
        Err(e) => AdminTaskResp {
            log: String::new(),
            success: false,
            error: Some(format!("Task panicked: {e}")),
        },
    };
    Ok(axum::Json(res))
}
