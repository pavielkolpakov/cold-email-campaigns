mod support;

use api::campaigns::{self, NewCampaign};
use api::leads::{self, ColumnMapping};
use api::sequences::{self, NewStep};
use sqlx::PgPool;
use support::{cipher, seed_org};
use uuid::Uuid;

async fn seed_sequence(pool: &PgPool, org_id: Uuid) -> Uuid {
    sequences::create(
        pool,
        org_id,
        "Outreach",
        vec![
            NewStep {
                delay_days: 0,
                subject: "Quick question, {{first_name}}".into(),
                body: "Hi {{first_name}}, saw {{company}} is hiring.".into(),
            },
            NewStep {
                delay_days: 3,
                subject: String::new(),
                body: "Bumping this to the top of your inbox.".into(),
            },
        ],
    )
    .await
    .unwrap()
    .id
}

async fn seed_leads(pool: &PgPool, org_id: Uuid, csv: &str) -> Uuid {
    let list_id = leads::create_list(pool, org_id, "Prospects").await.unwrap().id;
    leads::import_csv(
        pool,
        org_id,
        list_id,
        csv.as_bytes(),
        &ColumnMapping {
            email: "Email".into(),
            first_name: Some("First".into()),
            last_name: None,
            company: Some("Company".into()),
        },
    )
    .await
    .unwrap();
    list_id
}

const THREE_LEADS: &str = "Email,First,Company\n\
     ada@example.com,Ada,Analytical Engines\n\
     grace@example.com,Grace,US Navy\n\
     alan@example.com,Alan,Bletchley Park\n";

async fn seed_mailbox(pool: &PgPool, org_id: Uuid) -> Uuid {
    use api::mailboxes::{self, Credentials, NewMailbox};
    use chrono::{Duration, Utc};

    mailboxes::connect(
        pool,
        &cipher(),
        org_id,
        NewMailbox {
            provider: "gmail".into(),
            email: "sender@acme.com".into(),
            credentials: Credentials {
                access_token: "ya29.token".into(),
                refresh_token: "1//refresh".into(),
                expires_at: Utc::now() + Duration::hours(1),
            },
        },
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn launching_enrolls_every_lead_at_the_first_step(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let campaign = campaigns::create(
        &pool,
        org_id,
        NewCampaign {
            name: "Q4 outreach".into(),
            sequence_id: seed_sequence(&pool, org_id).await,
            list_id: seed_leads(&pool, org_id, THREE_LEADS).await,
            mailbox_id: seed_mailbox(&pool, org_id).await,
        },
    )
    .await
    .unwrap();
    assert_eq!(campaign.status, "draft");

    let enrolled = campaigns::launch(&pool, org_id, campaign.id).await.unwrap();

    assert_eq!(enrolled, 3);
    let stats = campaigns::stats(&pool, org_id, campaign.id).await.unwrap();
    assert_eq!(stats.total, 3);
    assert_eq!(stats.pending, 3);
    assert_eq!(stats.sent, 0);
}

#[sqlx::test]
async fn suppressed_and_unsubscribed_leads_are_never_enrolled(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let list_id = seed_leads(&pool, org_id, THREE_LEADS).await;

    api::suppressions::add(&pool, org_id, "grace@example.com", "manual").await.unwrap();
    sqlx::query!("update leads set status = 'unsubscribed' where email = 'alan@example.com'")
        .execute(&pool)
        .await
        .unwrap();

    let campaign = campaigns::create(
        &pool,
        org_id,
        NewCampaign {
            name: "Q4 outreach".into(),
            sequence_id: seed_sequence(&pool, org_id).await,
            list_id,
            mailbox_id: seed_mailbox(&pool, org_id).await,
        },
    )
    .await
    .unwrap();

    let enrolled = campaigns::launch(&pool, org_id, campaign.id).await.unwrap();

    assert_eq!(enrolled, 1, "only ada is still mailable");
}

async fn launched_campaign(pool: &PgPool, org_id: Uuid) -> Uuid {
    let campaign = campaigns::create(
        pool,
        org_id,
        NewCampaign {
            name: "Q4 outreach".into(),
            sequence_id: seed_sequence(pool, org_id).await,
            list_id: seed_leads(pool, org_id, THREE_LEADS).await,
            mailbox_id: seed_mailbox(pool, org_id).await,
        },
    )
    .await
    .unwrap();
    campaigns::launch(pool, org_id, campaign.id).await.unwrap();
    campaign.id
}

#[sqlx::test]
async fn the_scheduler_enqueues_each_due_lead_exactly_once(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    launched_campaign(&pool, org_id).await;

    let enqueued = api::scheduler::enqueue_due(&pool).await.unwrap();
    assert_eq!(enqueued, 3);

    // Running again must not double-send to anyone.
    let again = api::scheduler::enqueue_due(&pool).await.unwrap();
    assert_eq!(again, 0);

    let pending = sqlx::query_scalar!("select count(*) from jobs where status = 'pending'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pending, Some(3));
}

#[sqlx::test]
async fn a_paused_campaign_enqueues_nothing(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let campaign_id = launched_campaign(&pool, org_id).await;

    campaigns::pause(&pool, org_id, campaign_id).await.unwrap();
    assert_eq!(api::scheduler::enqueue_due(&pool).await.unwrap(), 0);

    campaigns::resume(&pool, org_id, campaign_id).await.unwrap();
    assert_eq!(api::scheduler::enqueue_due(&pool).await.unwrap(), 3);
}

use api::provider::RefreshedToken;
use chrono::{Duration as ChronoDuration, Utc};
use std::sync::Arc;
use support::FakeMailer;

/// The stored access token is still valid in these tests, so refresh is never called.
struct NeverRefreshes;

#[async_trait::async_trait]
impl api::provider::TokenRefresher for NeverRefreshes {
    async fn refresh(&self, _: &str) -> Result<RefreshedToken, api::provider::RefreshError> {
        panic!("a valid token must not be refreshed");
    }
}

#[sqlx::test]
async fn the_worker_sends_renders_and_schedules_the_followup(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let campaign_id = launched_campaign(&pool, org_id).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();

    let mailer = Arc::new(FakeMailer::default());
    let processed = api::worker::run_once(
        &pool,
        &cipher(),
        &NeverRefreshes,
        mailer.as_ref(),
        "worker-1",
        10,
    )
    .await
    .unwrap();

    assert_eq!(processed, 3);

    let sent = mailer.sent.lock().unwrap();
    assert_eq!(sent.len(), 3);
    let ada = sent
        .iter()
        .find(|(_, message)| message.to == "ada@example.com")
        .expect("ada should have been mailed");
    assert_eq!(ada.1.subject, "Quick question, Ada");
    assert!(
        ada.1.body.starts_with("Hi Ada, saw Analytical Engines is hiring."),
        "body was {}",
        ada.1.body
    );

    // Everyone moves to step 2, due in three days, still pending.
    let rows = sqlx::query!(
        "select current_step, next_run_at, status, thread_id from campaign_leads where campaign_id = $1",
        campaign_id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    for row in &rows {
        assert_eq!(row.current_step, 1);
        assert_eq!(row.status, "pending");
        assert!(row.thread_id.is_some(), "the thread must be kept for followups");
        let due = row.next_run_at.expect("a followup must be scheduled");
        let expected = Utc::now() + ChronoDuration::days(3);
        assert!(
            (due - expected).num_minutes().abs() < 5,
            "followup due {due}, expected around {expected}"
        );
    }

    let messages = sqlx::query_scalar!("select count(*) from messages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(messages, Some(3));
}

async fn work(pool: &PgPool, mailer: &FakeMailer) -> usize {
    api::worker::run_once(pool, &cipher(), &NeverRefreshes, mailer, "worker-1", 10)
        .await
        .unwrap()
}

/// Pretends the followup delay has elapsed.
async fn make_everything_due(pool: &PgPool) {
    sqlx::query!("update campaign_leads set next_run_at = now() where status = 'pending'")
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test]
async fn a_lead_is_finished_after_the_last_step(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let campaign_id = launched_campaign(&pool, org_id).await;
    let mailer = FakeMailer::default();

    api::scheduler::enqueue_due(&pool).await.unwrap();
    work(&pool, &mailer).await;

    make_everything_due(&pool).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();
    work(&pool, &mailer).await;

    assert_eq!(mailer.sent.lock().unwrap().len(), 6, "two steps to three leads");

    let stats = campaigns::stats(&pool, org_id, campaign_id).await.unwrap();
    assert_eq!(stats.pending, 0);
    assert_eq!(stats.sent, 3);

    // Nothing is left to schedule.
    assert_eq!(api::scheduler::enqueue_due(&pool).await.unwrap(), 0);
}

#[sqlx::test]
async fn the_followup_replies_inside_the_first_thread(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    launched_campaign(&pool, org_id).await;
    let mailer = FakeMailer::default();

    api::scheduler::enqueue_due(&pool).await.unwrap();
    work(&pool, &mailer).await;
    make_everything_due(&pool).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();
    work(&pool, &mailer).await;

    let sent = mailer.sent.lock().unwrap();
    let followups: Vec<_> = sent
        .iter()
        .filter(|(_, message)| message.in_reply_to.is_some())
        .collect();
    assert_eq!(followups.len(), 3, "every followup must thread");
    assert_eq!(followups[0].1.thread_id.as_deref(), Some("thread-1"));
}

#[sqlx::test]
async fn the_daily_cap_stops_the_mailbox(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let campaign_id = launched_campaign(&pool, org_id).await;
    sqlx::query!("update mailboxes set daily_cap = 2")
        .execute(&pool)
        .await
        .unwrap();
    let mailer = FakeMailer::default();

    api::scheduler::enqueue_due(&pool).await.unwrap();
    work(&pool, &mailer).await;

    assert_eq!(mailer.sent.lock().unwrap().len(), 2, "the cap is a hard stop");

    // The third is retried later, not dropped or failed.
    let stats = campaigns::stats(&pool, org_id, campaign_id).await.unwrap();
    assert_eq!(stats.failed, 0);
    let retrying = sqlx::query_scalar!("select count(*) from jobs where status = 'pending'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(retrying, Some(1));
}

#[sqlx::test]
async fn a_lead_missing_a_merge_value_is_never_mailed(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    // No Company column at all, and the first step's body requires {{company}}.
    let list_id = {
        let list_id = leads::create_list(&pool, org_id, "Prospects").await.unwrap().id;
        leads::import_csv(
            &pool,
            org_id,
            list_id,
            b"Email,First\nada@example.com,Ada\n",
            &ColumnMapping {
                email: "Email".into(),
                first_name: Some("First".into()),
                last_name: None,
                company: Some("Company".into()),
            },
        )
        .await
        .unwrap();
        list_id
    };

    let campaign = campaigns::create(
        &pool,
        org_id,
        NewCampaign {
            name: "Q4 outreach".into(),
            sequence_id: seed_sequence(&pool, org_id).await,
            list_id,
            mailbox_id: seed_mailbox(&pool, org_id).await,
        },
    )
    .await
    .unwrap();
    campaigns::launch(&pool, org_id, campaign.id).await.unwrap();
    api::scheduler::enqueue_due(&pool).await.unwrap();

    let mailer = FakeMailer::default();
    work(&pool, &mailer).await;

    assert!(
        mailer.sent.lock().unwrap().is_empty(),
        "`saw  is hiring` must never reach a prospect"
    );

    // Permanent: retrying cannot conjure a value, so it fails immediately.
    let stats = campaigns::stats(&pool, org_id, campaign.id).await.unwrap();
    assert_eq!(stats.failed, 1);
    let error = sqlx::query_scalar!("select last_error from campaign_leads limit 1")
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap();
    assert!(error.contains("company"), "the error must name the tag: {error}");
}

#[sqlx::test]
async fn unsubscribing_after_scheduling_cancels_the_send(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    launched_campaign(&pool, org_id).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();

    // The job is already queued when they click unsubscribe.
    api::suppressions::add(&pool, org_id, "ada@example.com", "unsubscribed")
        .await
        .unwrap();

    let mailer = FakeMailer::default();
    work(&pool, &mailer).await;

    let sent = mailer.sent.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert!(
        !sent.iter().any(|(_, message)| message.to == "ada@example.com"),
        "a queued job must not outrun an unsubscribe"
    );
}

#[sqlx::test]
async fn two_workers_never_send_the_same_email_twice(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    launched_campaign(&pool, org_id).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();

    let first = Arc::new(FakeMailer::default());
    let second = Arc::new(FakeMailer::default());

    let cipher = cipher();
    let (a, b) = tokio::join!(
        api::worker::run_once(&pool, &cipher, &NeverRefreshes, first.as_ref(), "worker-a", 10),
        api::worker::run_once(&pool, &cipher, &NeverRefreshes, second.as_ref(), "worker-b", 10),
    );

    assert_eq!(a.unwrap() + b.unwrap(), 3);

    let mut recipients: Vec<String> = first
        .sent
        .lock()
        .unwrap()
        .iter()
        .chain(second.sent.lock().unwrap().iter())
        .map(|(_, message)| message.to.clone())
        .collect();
    recipients.sort();
    let unique = {
        let mut copy = recipients.clone();
        copy.dedup();
        copy
    };
    assert_eq!(recipients, unique, "a prospect must not get two copies");
    assert_eq!(recipients.len(), 3);
}

#[sqlx::test]
async fn every_send_carries_a_working_unsubscribe_link(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    launched_campaign(&pool, org_id).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();

    let mailer = FakeMailer::default();
    work(&pool, &mailer).await;

    let body = {
        let sent = mailer.sent.lock().unwrap();
        sent.iter()
            .find(|(_, message)| message.to == "ada@example.com")
            .unwrap()
            .1
            .body
            .clone()
    };
    assert!(
        body.to_lowercase().contains("unsubscribe"),
        "no opt-out in: {body}"
    );

    // Pull the token out of the link exactly as a recipient's mail client would.
    let token = body
        .split("/u/")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("an unsubscribe url")
        .trim_end_matches(['.', ')'])
        .to_string();

    api::suppressions::unsubscribe(&pool, &token).await.unwrap();

    assert!(
        api::suppressions::is_suppressed(&pool, org_id, "ada@example.com")
            .await
            .unwrap()
    );

    // And the queued followup is cancelled.
    make_everything_due(&pool).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();
    let mailer = FakeMailer::default();
    work(&pool, &mailer).await;
    assert!(
        !mailer
            .sent
            .lock()
            .unwrap()
            .iter()
            .any(|(_, message)| message.to == "ada@example.com")
    );
}

#[sqlx::test]
async fn a_followup_carries_what_gmail_needs_to_thread_it(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    launched_campaign(&pool, org_id).await;
    let mailer = FakeMailer::default();

    api::scheduler::enqueue_due(&pool).await.unwrap();
    work(&pool, &mailer).await;
    make_everything_due(&pool).await;
    api::scheduler::enqueue_due(&pool).await.unwrap();
    work(&pool, &mailer).await;

    let sent = mailer.sent.lock().unwrap();
    let to_ada: Vec<_> = sent
        .iter()
        .filter(|(_, message)| message.to == "ada@example.com")
        .map(|(_, message)| message)
        .collect();
    assert_eq!(to_ada.len(), 2);
    let (first, followup) = (to_ada[0], to_ada[1]);

    // The opening message starts a thread and must not claim to reply to anything.
    assert!(first.in_reply_to.is_none());
    assert!(first.thread_id.is_none());

    // Gmail needs the API thread id *and* the RFC Message-ID of what we are
    // replying to. The thread id alone silently starts a new conversation.
    assert_eq!(followup.thread_id.as_deref(), Some("thread-1"));
    assert_eq!(
        followup.in_reply_to.as_deref(),
        Some("<message-id-1@test>"),
        "In-Reply-To must be the original Message-ID header, not the thread id"
    );

    // Mail clients also group on subject, so a followup reuses the original.
    assert_eq!(followup.subject, "Re: Quick question, Ada");
}
