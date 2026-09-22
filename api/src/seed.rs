use anyhow::Result;
use sqlx::PgPool;

use crate::auth::hash_password;
use crate::leads::{self, ColumnMapping};
use crate::sequences::{self, NewStep};

const DEMO_EMAIL: &str = "demo@example.com";
const DEMO_PASSWORD: &str = "demo-password-123";

const LEADS_CSV: &str = "Email,First,Last,Company,Title\n\
ada@example.com,Ada,Lovelace,Analytical Engines,CTO\n\
grace@example.com,Grace,Hopper,US Navy,Rear Admiral\n\
alan@example.com,Alan,Turing,Bletchley Park,Cryptanalyst\n\
katherine@example.com,Katherine,Johnson,NASA,Mathematician\n";

/// Fills an empty database with something to click around in. Deliberately
/// goes through the same functions the app uses, so it cannot drift from them.
/// A mailbox cannot be seeded — that needs a real OAuth grant.
pub async fn run(pool: &PgPool) -> Result<()> {
    if sqlx::query_scalar!(
        "select exists (select 1 from users where email = $1)",
        DEMO_EMAIL
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false)
    {
        println!("Demo data already present. Sign in as {DEMO_EMAIL} / {DEMO_PASSWORD}");
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    let org_id = sqlx::query_scalar!("insert into orgs (name) values ('Demo') returning id")
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query!(
        "insert into users (org_id, email, password_hash, name, role)
         values ($1, $2, $3, 'Demo Owner', 'owner')",
        org_id,
        DEMO_EMAIL,
        hash_password(DEMO_PASSWORD).map_err(|err| anyhow::anyhow!("{err}"))?,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let list = leads::create_list(pool, org_id, "Demo prospects").await?;
    let report = leads::import_csv(
        pool,
        org_id,
        list.id,
        LEADS_CSV.as_bytes(),
        &ColumnMapping {
            email: "Email".into(),
            first_name: Some("First".into()),
            last_name: Some("Last".into()),
            company: Some("Company".into()),
        },
    )
    .await?;

    sequences::create(
        pool,
        org_id,
        "Demo outreach",
        vec![
            NewStep {
                delay_days: 0,
                subject: "Quick question, {{first_name|there}}".into(),
                body: "Hi {{first_name|there}}, noticed {{company|your team}} is growing. \
                       Worth a short chat?"
                    .into(),
            },
            NewStep {
                delay_days: 3,
                subject: String::new(),
                body: "Bumping this in case it got buried.".into(),
            },
        ],
    )
    .await?;

    println!(
        "Seeded org 'Demo' with {} leads and one sequence.",
        report.imported
    );
    println!("Sign in as {DEMO_EMAIL} / {DEMO_PASSWORD}");
    println!("Connect a Gmail mailbox to launch a campaign.");
    Ok(())
}
