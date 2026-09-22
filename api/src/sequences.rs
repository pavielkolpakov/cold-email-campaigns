use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Sequence {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Step {
    pub id: Uuid,
    pub position: i32,
    pub delay_days: i32,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NewStep {
    pub delay_days: i32,
    pub subject: String,
    pub body: String,
}

pub async fn create(
    pool: &PgPool,
    org_id: Uuid,
    name: &str,
    steps: Vec<NewStep>,
) -> Result<Sequence> {
    let mut tx = pool.begin().await?;

    let sequence = sqlx::query_as!(
        Sequence,
        "insert into sequences (org_id, name) values ($1, $2) returning id, org_id, name",
        org_id,
        name.trim(),
    )
    .fetch_one(&mut *tx)
    .await?;

    for (position, step) in steps.into_iter().enumerate() {
        sqlx::query!(
            r#"
            insert into sequence_steps (sequence_id, position, delay_days, subject, body)
            values ($1, $2, $3, $4, $5)
            "#,
            sequence.id,
            position as i32,
            step.delay_days,
            step.subject,
            step.body,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(sequence)
}

pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<Sequence>> {
    let sequences = sqlx::query_as!(
        Sequence,
        "select id, org_id, name from sequences where org_id = $1 order by created_at desc",
        org_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(sequences)
}

pub async fn steps(pool: &PgPool, org_id: Uuid, sequence_id: Uuid) -> Result<Vec<Step>> {
    let steps = sqlx::query_as!(
        Step,
        r#"
        select s.id, s.position, s.delay_days, s.subject, s.body
        from sequence_steps s
        join sequences q on q.id = s.sequence_id
        where s.sequence_id = $1 and q.org_id = $2
        order by s.position
        "#,
        sequence_id,
        org_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(steps)
}
