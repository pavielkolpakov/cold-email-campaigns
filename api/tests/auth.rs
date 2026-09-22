mod support;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

fn app(pool: PgPool) -> Router {
    support::test_app(pool, std::sync::Arc::new(support::FakeMailer::default()))
}

async fn post(app: &Router, path: &str, body: Value) -> (StatusCode, Option<String>, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .map(|value| value.to_str().unwrap().to_string());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, cookie, json)
}

async fn get_me(app: &Router, cookie: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder().method("GET").uri("/auth/me");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn signup_body(org: &str, email: &str) -> Value {
    json!({
        "org_name": org,
        "name": "Test User",
        "email": email,
        "password": "correct-horse-battery",
    })
}

#[sqlx::test]
async fn signup_creates_an_org_and_signs_the_user_in(pool: PgPool) {
    let app = app(pool);

    let (status, cookie, body) =
        post(&app, "/auth/signup", signup_body("Acme", "ada@acme.com")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["role"], "owner");
    assert_eq!(body["email"], "ada@acme.com");

    let cookie = cookie.expect("signup should set a session cookie");
    assert!(
        cookie.contains("HttpOnly"),
        "session cookie must be HttpOnly"
    );
    assert!(cookie.contains("SameSite=Lax"));

    let (status, me) = get_me(&app, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["id"], body["id"]);
}

#[sqlx::test]
async fn signup_normalizes_email_and_rejects_duplicates(pool: PgPool) {
    let app = app(pool);

    let (status, _, body) = post(&app, "/auth/signup", signup_body("Acme", "Ada@Acme.com")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], "ada@acme.com");

    let (status, _, body) = post(&app, "/auth/signup", signup_body("Other", "ADA@acme.com")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().unwrap().contains("already exists"));
}

#[sqlx::test]
async fn signup_rejects_a_short_password(pool: PgPool) {
    let app = app(pool);

    let (status, cookie, _) = post(
        &app,
        "/auth/signup",
        json!({"org_name": "Acme", "name": "Ada", "email": "ada@acme.com", "password": "short"}),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(cookie.is_none());
}

#[sqlx::test]
async fn login_rejects_a_wrong_password(pool: PgPool) {
    let app = app(pool);
    post(&app, "/auth/signup", signup_body("Acme", "ada@acme.com")).await;

    let (status, cookie, _) = post(
        &app,
        "/auth/login",
        json!({"email": "ada@acme.com", "password": "not-the-password"}),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(cookie.is_none());
}

#[sqlx::test]
async fn me_requires_a_session(pool: PgPool) {
    let app = app(pool);

    let (status, _) = get_me(&app, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = get_me(&app, Some("ce_session=not-a-uuid")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn logout_invalidates_the_session(pool: PgPool) {
    let app = app(pool);
    let (_, cookie, _) = post(&app, "/auth/signup", signup_body("Acme", "ada@acme.com")).await;
    let cookie = cookie.unwrap();

    let (status, _, _) = post(&app, "/auth/logout", json!({})).await;
    assert_eq!(status, StatusCode::OK);

    // Logging out without the cookie must not invalidate anyone else's session.
    let (status, _) = get_me(&app, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (status, _) = get_me(&app, Some(&cookie)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Two orgs created through signup must never share a tenant boundary.
#[sqlx::test]
async fn separate_signups_get_separate_orgs(pool: PgPool) {
    let app = app(pool);

    let (_, _, acme) = post(&app, "/auth/signup", signup_body("Acme", "ada@acme.com")).await;
    let (_, _, globex) = post(
        &app,
        "/auth/signup",
        signup_body("Globex", "hank@globex.com"),
    )
    .await;

    assert_ne!(acme["org_id"], globex["org_id"]);
}

#[sqlx::test]
async fn google_sign_in_creates_an_account_once(pool: PgPool) {
    let first = api::auth::sign_in_with_google(&pool, "Grace@Navy.mil", "Grace Hopper")
        .await
        .unwrap();
    assert_eq!(first.email, "grace@navy.mil");
    assert_eq!(first.role, "owner");

    let again = api::auth::sign_in_with_google(&pool, "grace@navy.mil", "Grace Hopper")
        .await
        .unwrap();
    assert_eq!(again.id, first.id);

    let orgs = sqlx::query_scalar!("select count(*) from orgs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(orgs, Some(1));
}

#[sqlx::test]
async fn google_sign_in_reuses_a_password_account(pool: PgPool) {
    let app = app(pool.clone());
    let (_, _, body) = post(&app, "/auth/signup", signup_body("Acme", "ada@acme.com")).await;

    let user = api::auth::sign_in_with_google(&pool, "ada@acme.com", "Ada")
        .await
        .unwrap();
    assert_eq!(user.id.to_string(), body["id"]);
}

#[sqlx::test]
async fn a_google_only_account_cannot_log_in_with_a_password(pool: PgPool) {
    api::auth::sign_in_with_google(&pool, "grace@navy.mil", "Grace")
        .await
        .unwrap();

    let (status, cookie, _) = post(
        &app(pool),
        "/auth/login",
        json!({ "email": "grace@navy.mil", "password": "" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(cookie.is_none());
}

async fn get_raw(app: &Router, uri: &str, cookie: Option<&str>) -> axum::response::Response {
    let mut request = Request::builder().method("GET").uri(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[sqlx::test]
async fn google_authorize_redirects_with_a_state_cookie(pool: PgPool) {
    let response = get_raw(&app(pool), "/auth/google/authorize", None).await;
    assert!(response.status().is_redirection());

    let location = response.headers()[header::LOCATION].to_str().unwrap();
    assert!(location.starts_with("https://accounts.google.com/"));
    assert!(location.contains("scope=openid"));
    assert!(location.contains("api%2Fauth%2Fgoogle%2Fcallback"));

    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with("ce_google_sign_in_state="));
}

#[sqlx::test]
async fn google_callback_rejects_a_forged_state(pool: PgPool) {
    let response = get_raw(
        &app(pool),
        "/auth/google/callback?code=abc&state=forged",
        Some("ce_google_sign_in_state=expected"),
    )
    .await;

    let location = response.headers()[header::LOCATION].to_str().unwrap();
    assert_eq!(location, "http://localhost:3000/login?error=google_expired");
    let sets_session = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .any(|value| value.to_str().unwrap().starts_with("ce_session="));
    assert!(!sets_session);
}
