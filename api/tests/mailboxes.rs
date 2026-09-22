mod support;

use api::mailboxes::{self, Credentials, NewMailbox};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use support::{cipher, seed_org};

const REFRESH_TOKEN: &str = "1//0gTheRefreshTokenGoogleGaveUs";

fn gmail_credentials() -> Credentials {
    Credentials {
        access_token: "ya29.the-access-token".into(),
        refresh_token: REFRESH_TOKEN.into(),
        expires_at: Utc::now() + Duration::hours(1),
    }
}

fn new_mailbox(email: &str) -> NewMailbox {
    NewMailbox {
        provider: "gmail".into(),
        email: email.into(),
        credentials: gmail_credentials(),
    }
}

#[sqlx::test]
async fn connecting_a_mailbox_keeps_its_tokens_out_of_the_database(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;

    let mailbox = mailboxes::connect(&pool, &cipher(), org_id, new_mailbox("ada@acme.com"))
        .await
        .unwrap();

    // Whatever the schema looks like, the secret must not be sitting in it.
    let dump: String = sqlx::query_scalar("select mailboxes::text from mailboxes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !dump.contains(REFRESH_TOKEN),
        "refresh token found in plaintext: {dump}"
    );

    let credentials = mailboxes::credentials(&pool, &cipher(), org_id, mailbox.id)
        .await
        .unwrap();
    assert_eq!(credentials.refresh_token, REFRESH_TOKEN);
}

#[sqlx::test]
async fn another_org_cannot_read_or_list_a_mailbox(pool: PgPool) {
    let acme = seed_org(&pool, "Acme").await;
    let globex = seed_org(&pool, "Globex").await;

    let mailbox = mailboxes::connect(&pool, &cipher(), acme, new_mailbox("ada@acme.com"))
        .await
        .unwrap();

    assert!(
        mailboxes::credentials(&pool, &cipher(), globex, mailbox.id)
            .await
            .is_err(),
        "globex must not be able to read acme's tokens"
    );
    assert!(mailboxes::list(&pool, globex).await.unwrap().is_empty());
    assert_eq!(mailboxes::list(&pool, acme).await.unwrap().len(), 1);
}

use api::provider::gmail::GmailOAuth;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn oauth_server(response: ResponseTemplate, expected_calls: u64) -> (MockServer, GmailOAuth) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .respond_with(response)
        .expect(expected_calls)
        .mount(&server)
        .await;

    let oauth = GmailOAuth::new(
        "client-id".into(),
        "client-secret".into(),
        format!("{}/token", server.uri()),
    );
    (server, oauth)
}

#[sqlx::test]
async fn an_expired_access_token_is_refreshed_and_persisted(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let (_server, oauth) = oauth_server(
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "ya29.the-refreshed-token",
            "expires_in": 3599,
            "token_type": "Bearer",
        })),
        1,
    )
    .await;

    let mut expired = new_mailbox("ada@acme.com");
    expired.credentials.expires_at = Utc::now() - Duration::minutes(5);
    let mailbox = mailboxes::connect(&pool, &cipher(), org_id, expired)
        .await
        .unwrap();

    let fresh = mailboxes::fresh_credentials(&pool, &cipher(), &oauth, org_id, mailbox.id)
        .await
        .unwrap();
    assert_eq!(fresh.access_token, "ya29.the-refreshed-token");
    assert!(fresh.expires_at > Utc::now());
    assert_eq!(
        fresh.refresh_token, REFRESH_TOKEN,
        "google does not reissue the refresh token, so we must keep ours"
    );

    // Persisted, so the next call does not hit Google again (the mock expects exactly one call).
    let again = mailboxes::fresh_credentials(&pool, &cipher(), &oauth, org_id, mailbox.id)
        .await
        .unwrap();
    assert_eq!(again.access_token, "ya29.the-refreshed-token");
}

#[sqlx::test]
async fn a_revoked_grant_marks_the_mailbox_disconnected(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let (_server, oauth) = oauth_server(
        ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant",
            "error_description": "Token has been expired or revoked.",
        })),
        1,
    )
    .await;

    let mut expired = new_mailbox("ada@acme.com");
    expired.credentials.expires_at = Utc::now() - Duration::minutes(5);
    let mailbox = mailboxes::connect(&pool, &cipher(), org_id, expired)
        .await
        .unwrap();

    assert!(
        mailboxes::fresh_credentials(&pool, &cipher(), &oauth, org_id, mailbox.id)
            .await
            .is_err()
    );

    let status = sqlx::query_scalar!("select status from mailboxes where id = $1", mailbox.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "disconnected",
        "the user must be told to reconnect, not silently retried forever"
    );
}

#[sqlx::test]
async fn a_transient_refresh_failure_leaves_the_mailbox_active(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let (_server, oauth) = oauth_server(ResponseTemplate::new(503), 1).await;

    let mut expired = new_mailbox("ada@acme.com");
    expired.credentials.expires_at = Utc::now() - Duration::minutes(5);
    let mailbox = mailboxes::connect(&pool, &cipher(), org_id, expired)
        .await
        .unwrap();

    assert!(
        mailboxes::fresh_credentials(&pool, &cipher(), &oauth, org_id, mailbox.id)
            .await
            .is_err()
    );

    let status = sqlx::query_scalar!("select status from mailboxes where id = $1", mailbox.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "active",
        "google being down is not the user's problem"
    );
}

use support::FakeMailer;

#[sqlx::test]
async fn a_test_send_uses_a_freshly_refreshed_token(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let (_server, oauth) = oauth_server(
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "ya29.the-refreshed-token",
            "expires_in": 3599,
        })),
        1,
    )
    .await;
    let mailer = FakeMailer::default();

    let mut expired = new_mailbox("ada@acme.com");
    expired.credentials.expires_at = Utc::now() - Duration::minutes(5);
    let mailbox = mailboxes::connect(&pool, &cipher(), org_id, expired)
        .await
        .unwrap();

    mailboxes::send_test_email(&pool, &cipher(), &oauth, &mailer, org_id, mailbox.id)
        .await
        .unwrap();

    let sent = mailer.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0, "ya29.the-refreshed-token",
        "the stale token must never reach the provider"
    );
    assert_eq!(sent[0].1.to, "ada@acme.com");
}

#[sqlx::test]
async fn another_org_cannot_send_a_test_email(pool: PgPool) {
    let acme = seed_org(&pool, "Acme").await;
    let globex = seed_org(&pool, "Globex").await;
    let (_server, oauth) = oauth_server(ResponseTemplate::new(200), 0).await;
    let mailer = FakeMailer::default();

    let mailbox = mailboxes::connect(&pool, &cipher(), acme, new_mailbox("ada@acme.com"))
        .await
        .unwrap();

    assert!(
        mailboxes::send_test_email(&pool, &cipher(), &oauth, &mailer, globex, mailbox.id)
            .await
            .is_err()
    );
    assert!(mailer.sent.lock().unwrap().is_empty());
}
