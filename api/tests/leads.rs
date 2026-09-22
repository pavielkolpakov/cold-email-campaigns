mod support;

use api::leads::{self, ColumnMapping};
use sqlx::PgPool;
use support::seed_org;
use uuid::Uuid;

fn mapping() -> ColumnMapping {
    ColumnMapping {
        email: "Email".into(),
        first_name: Some("First".into()),
        last_name: Some("Last".into()),
        company: Some("Company".into()),
    }
}

async fn seed_list(pool: &PgPool, org_id: Uuid) -> Uuid {
    leads::create_list(pool, org_id, "Q4 prospects").await.unwrap().id
}

#[sqlx::test]
async fn importing_a_csv_maps_columns_and_keeps_the_extras(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let list_id = seed_list(&pool, org_id).await;

    let csv = "Email,First,Last,Company,Title,Industry\n\
               ada@example.com,Ada,Lovelace,Analytical Engines,CTO,Software\n";

    let report = leads::import_csv(&pool, org_id, list_id, csv.as_bytes(), &mapping())
        .await
        .unwrap();

    assert_eq!(report.imported, 1);
    assert!(report.errors.is_empty());

    let lead = leads::list(&pool, org_id, list_id).await.unwrap().remove(0);
    assert_eq!(lead.email, "ada@example.com");
    assert_eq!(lead.first_name.as_deref(), Some("Ada"));
    assert_eq!(lead.company.as_deref(), Some("Analytical Engines"));
    assert_eq!(lead.custom["Title"], "CTO");
    assert_eq!(lead.custom["Industry"], "Software");
}

#[sqlx::test]
async fn emails_are_normalized_and_imported_only_once_per_org(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let list_id = seed_list(&pool, org_id).await;

    let csv = "Email,First,Last,Company\n\
               Ada@Example.com,Ada,Lovelace,Analytical Engines\n\
                 ada@example.com  ,Ada,Lovelace,Analytical Engines\n\
               grace@example.com,Grace,Hopper,Navy\n";

    let report = leads::import_csv(&pool, org_id, list_id, csv.as_bytes(), &mapping())
        .await
        .unwrap();

    assert_eq!(report.imported, 2);
    assert_eq!(report.duplicates, 1);
    assert!(report.errors.is_empty());

    let emails: Vec<String> = leads::list(&pool, org_id, list_id)
        .await
        .unwrap()
        .into_iter()
        .map(|lead| lead.email)
        .collect();
    assert_eq!(emails, vec!["ada@example.com", "grace@example.com"]);

    // A second import of someone already on file is a duplicate, not an error.
    let again = leads::import_csv(&pool, org_id, list_id, csv.as_bytes(), &mapping())
        .await
        .unwrap();
    assert_eq!(again.imported, 0);
    assert_eq!(again.duplicates, 3);
}

#[sqlx::test]
async fn bad_rows_are_reported_by_row_number_without_losing_the_good_ones(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let list_id = seed_list(&pool, org_id).await;

    let csv = "Email,First,Last,Company\n\
               ,Nobody,Nowhere,Nothing\n\
               not-an-email,Bad,Row,Corp\n\
               grace@example.com,Grace,Hopper,Navy\n";

    let report = leads::import_csv(&pool, org_id, list_id, csv.as_bytes(), &mapping())
        .await
        .unwrap();

    assert_eq!(report.imported, 1);
    assert_eq!(report.errors.len(), 2);
    assert_eq!(report.errors[0].row, 2);
    assert!(report.errors[0].message.contains("email"));
    assert_eq!(report.errors[1].row, 3);

    let leads = leads::list(&pool, org_id, list_id).await.unwrap();
    assert_eq!(leads.len(), 1);
    assert_eq!(leads[0].email, "grace@example.com");
}

#[sqlx::test]
async fn leads_and_lists_do_not_cross_org_boundaries(pool: PgPool) {
    let acme = seed_org(&pool, "Acme").await;
    let globex = seed_org(&pool, "Globex").await;
    let acme_list = seed_list(&pool, acme).await;

    let csv = "Email,First,Last,Company\nada@example.com,Ada,Lovelace,Analytical Engines\n";
    leads::import_csv(&pool, acme, acme_list, csv.as_bytes(), &mapping())
        .await
        .unwrap();

    // Globex holds a valid list id of Acme's — it must still see nothing.
    assert!(leads::list(&pool, globex, acme_list).await.unwrap().is_empty());
    assert!(leads::lists(&pool, globex).await.unwrap().is_empty());
    assert_eq!(leads::lists(&pool, acme).await.unwrap().len(), 1);

    // And must not be able to write into it.
    assert!(
        leads::import_csv(&pool, globex, acme_list, csv.as_bytes(), &mapping())
            .await
            .is_err(),
        "globex must not be able to import into acme's list"
    );
    assert_eq!(leads::list(&pool, acme, acme_list).await.unwrap().len(), 1);
}

#[sqlx::test]
async fn an_imported_lead_exposes_every_column_as_a_merge_value(pool: PgPool) {
    let org_id = seed_org(&pool, "Acme").await;
    let list_id = seed_list(&pool, org_id).await;

    let csv = "Email,First,Last,Company,Title\n\
               ada@example.com,Ada,Lovelace,Analytical Engines,CTO\n";
    leads::import_csv(&pool, org_id, list_id, csv.as_bytes(), &mapping())
        .await
        .unwrap();

    let lead = leads::list(&pool, org_id, list_id).await.unwrap().remove(0);
    let values = lead.merge_values();

    let rendered = api::render::render(
        "Hi {{first_name}} ({{Title}}) at {{company}} — {{email}}",
        &values,
    )
    .unwrap();

    assert_eq!(
        rendered,
        "Hi Ada (CTO) at Analytical Engines — ada@example.com"
    );
}
