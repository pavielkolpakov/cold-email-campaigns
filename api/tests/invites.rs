mod support;

use api::invites;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::PgPool;
use support::seed_org;
use tower::ServiceExt;
use uuid::Uuid;

async fn seed_owner(pool: &PgPool, org_id: Uuid, email: &str) -> Uuid {
    sqlx::query_scalar!(
        "insert into users (org_id, email, password_hash, name, role)
         values ($1, $2, 'x', 'Owner', 'owner') returning id",
        org_id,
        email,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn an_accepted_invite_joins_the_inviting_org_as_a_member(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let owner = seed_owner(&pool, org_id, "ada@acme.com").await;

    let invite = invites::create(&pool, org_id, owner, "Grace@Acme.com", "member")
        .await
        .unwrap();
    assert_eq!(invite.email, "grace@acme.com");

    let user_id = invites::accept(&pool, &invite.token.to_string(), "Grace", "correct-horse-battery")
        .await
        .unwrap();

    let user = sqlx::query!("select org_id, email, role from users where id = $1", user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(user.org_id, org_id, "an invite decides the org, not the joiner");
    assert_eq!(user.email, "grace@acme.com");
    assert_eq!(user.role, "member");
}

#[sqlx::test]
async fn an_invite_can_only_be_used_once(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let owner = seed_owner(&pool, org_id, "ada@acme.com").await;
    let invite = invites::create(&pool, org_id, owner, "grace@acme.com", "member")
        .await
        .unwrap();
    let token = invite.token.to_string();

    invites::accept(&pool, &token, "Grace", "correct-horse-battery")
        .await
        .unwrap();

    assert!(
        invites::accept(&pool, &token, "Impostor", "correct-horse-battery")
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn an_expired_invite_is_refused(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let owner = seed_owner(&pool, org_id, "ada@acme.com").await;
    let invite = invites::create(&pool, org_id, owner, "grace@acme.com", "member")
        .await
        .unwrap();

    sqlx::query!("update invites set expires_at = now() - interval '1 day'")
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        invites::accept(&pool, &invite.token.to_string(), "Grace", "correct-horse-battery")
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn an_invite_cannot_be_revoked_by_another_org(pool: PgPool) {
    let acme = seed_org(&pool, "Acme").await;
    let globex = seed_org(&pool, "Globex").await;
    let owner = seed_owner(&pool, acme, "ada@acme.com").await;
    let invite = invites::create(&pool, acme, owner, "grace@acme.com", "member")
        .await
        .unwrap();

    assert!(invites::revoke(&pool, globex, invite.id).await.is_err());
    assert_eq!(invites::list(&pool, acme).await.unwrap().len(), 1);
}

#[sqlx::test]
async fn only_an_owner_can_invite(pool: PgPool) {
    let app = support::test_app(pool.clone(), std::sync::Arc::new(support::FakeMailer::default()));

    // Signing up makes an owner; they invite a member.
    let signup = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/signup")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"org_name":"Acme","name":"Ada","email":"ada@acme.com",
                           "password":"correct-horse-battery"})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let owner_cookie = signup
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let invited = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/invites")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &owner_cookie)
                .body(Body::from(
                    json!({"email":"grace@acme.com","role":"member"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invited.status(), StatusCode::OK);

    let body = invited.into_body().collect().await.unwrap().to_bytes();
    let token = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["invite"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let member_id = invites::accept(&pool, &token, "Grace", "correct-horse-battery")
        .await
        .unwrap();
    let session = api::auth::create_session(&pool, member_id, 30).await.unwrap();

    // The member tries to invite someone else.
    let refused = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/invites")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, format!("ce_session={session}"))
                .body(Body::from(
                    json!({"email":"alan@acme.com","role":"member"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
}
