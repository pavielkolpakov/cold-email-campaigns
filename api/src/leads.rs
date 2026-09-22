use anyhow::{Result, anyhow};
use serde_json::{Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LeadList {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Lead {
    pub id: Uuid,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub company: Option<String>,
    pub custom: Value,
    pub status: String,
}

/// Which CSV header feeds each known field. Everything else becomes `custom`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ColumnMapping {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub company: Option<String>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct ImportReport {
    pub imported: usize,
    /// Rows skipped because the org already holds that address.
    pub duplicates: usize,
    pub errors: Vec<RowError>,
}

#[derive(Debug, serde::Serialize)]
pub struct RowError {
    /// 1-based, counting the header as row 1, so it matches what a spreadsheet shows.
    pub row: usize,
    pub message: String,
}

/// Deliberately permissive: the only address we can truly validate is one that
/// accepts mail, so this rejects the obviously broken and lets the rest through.
fn is_plausible_email(email: &str) -> bool {
    let mut parts = email.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.contains(char::is_whitespace)
}

pub async fn create_list(pool: &PgPool, org_id: Uuid, name: &str) -> Result<LeadList> {
    let list = sqlx::query_as!(
        LeadList,
        "insert into lead_lists (org_id, name) values ($1, $2) returning id, org_id, name",
        org_id,
        name.trim(),
    )
    .fetch_one(pool)
    .await?;
    Ok(list)
}

pub async fn lists(pool: &PgPool, org_id: Uuid) -> Result<Vec<LeadList>> {
    let lists = sqlx::query_as!(
        LeadList,
        "select id, org_id, name from lead_lists where org_id = $1 order by created_at desc",
        org_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(lists)
}

pub async fn list(pool: &PgPool, org_id: Uuid, list_id: Uuid) -> Result<Vec<Lead>> {
    let leads = sqlx::query_as!(
        Lead,
        r#"
        select id, email, first_name, last_name, company, custom, status
        from leads
        where org_id = $1 and list_id = $2
        order by created_at, email
        "#,
        org_id,
        list_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(leads)
}

pub async fn import_csv(
    pool: &PgPool,
    org_id: Uuid,
    list_id: Uuid,
    csv_bytes: &[u8],
    mapping: &ColumnMapping,
) -> Result<ImportReport> {
    // The list id arrives from the request, so it is confirmed against the
    // caller's org before a single row is written.
    let owned = sqlx::query_scalar!(
        "select exists (select 1 from lead_lists where id = $1 and org_id = $2)",
        list_id,
        org_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if !owned {
        return Err(anyhow!("lead list {list_id} does not belong to this organization"));
    }

    let mut reader = csv::Reader::from_reader(csv_bytes);
    let headers = reader.headers()?.clone();
    let mut report = ImportReport::default();

    for (index, record) in reader.records().enumerate() {
        let row = index + 2; // header is row 1
        let record = record?;

        let parsed = match parse_row(&headers, &record, mapping) {
            Ok(parsed) => parsed,
            Err(message) => {
                report.errors.push(RowError { row, message });
                continue;
            }
        };

        let inserted = sqlx::query!(
            r#"
            insert into leads (org_id, list_id, email, first_name, last_name, company, custom)
            values ($1, $2, $3, $4, $5, $6, $7)
            on conflict do nothing
            "#,
            org_id,
            list_id,
            parsed.email,
            parsed.first_name,
            parsed.last_name,
            parsed.company,
            Value::Object(parsed.custom),
        )
        .execute(pool)
        .await?
        .rows_affected();

        if inserted == 0 {
            report.duplicates += 1;
        } else {
            report.imported += 1;
        }
    }

    Ok(report)
}

struct ParsedLead {
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    company: Option<String>,
    custom: Map<String, Value>,
}

/// Turns one CSV record into a lead, or explains why it cannot. Pure: no database,
/// so the mapping rules can be reasoned about on their own.
fn parse_row(
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
    mapping: &ColumnMapping,
) -> Result<ParsedLead, String> {
    let value = |header: &str| {
        headers
            .iter()
            .position(|candidate| candidate == header)
            .and_then(|position| record.get(position))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    let email = value(&mapping.email)
        .ok_or_else(|| "missing email".to_string())?
        .to_lowercase();
    if !is_plausible_email(&email) {
        return Err(format!("`{email}` is not a valid email address"));
    }

    let mapped: Vec<&str> = [
        Some(mapping.email.as_str()),
        mapping.first_name.as_deref(),
        mapping.last_name.as_deref(),
        mapping.company.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut custom = Map::new();
    for header in headers.iter() {
        if mapped.contains(&header) {
            continue;
        }
        if let Some(value) = value(header) {
            custom.insert(header.to_string(), Value::String(value));
        }
    }

    Ok(ParsedLead {
        first_name: mapping.first_name.as_deref().and_then(&value),
        last_name: mapping.last_name.as_deref().and_then(&value),
        company: mapping.company.as_deref().and_then(&value),
        email,
        custom,
    })
}
