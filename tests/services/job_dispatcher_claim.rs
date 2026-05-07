//! Regression tests for the queued→running atomic claim in
//! `workers::job_dispatcher`.
//!
//! Background: two callsites enqueue dispatcher work — `dispatch_queued_runs`
//! after every successful job, and the periodic `sweep_queued_runs` tokio
//! task. Without atomic claim, both can pick up the same row and execute
//! the same scan twice. These tests pin the atomic-claim contract.

use fracture_core::models::_entities::{job_definitions, job_runs, organizations};
use fracture_pt::app::App;
use fracture_pt::workers::job_dispatcher::claim_run;
use loco_rs::testing::prelude::*;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use serial_test::serial;
use uuid::Uuid;

async fn setup_queued_run(db: &DatabaseConnection) -> i32 {
    // Personal org for an arbitrary user; the run only needs an org_id +
    // job_definition_id that satisfy FKs.
    let org = organizations::ActiveModel {
        pid: Set(Uuid::new_v4()),
        name: Set("claim-test-org".into()),
        slug: Set(format!("claim-test-{}", Uuid::new_v4())),
        is_personal: Set(false),
        is_platform_admin: Set(false),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert org");

    let definition = job_definitions::ActiveModel {
        pid: Set(Uuid::new_v4()),
        org_id: Set(org.id),
        name: Set("claim-test-def".into()),
        job_type: Set("asm_scan".into()),
        schedule: Set(None),
        enabled: Set(true),
        config: Set("{}".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert definition");

    let run = job_runs::ActiveModel {
        pid: Set(Uuid::new_v4()),
        job_definition_id: Set(definition.id),
        org_id: Set(org.id),
        status: Set("queued".into()),
        started_at: Set(None),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert run");

    run.id
}

#[tokio::test]
#[serial]
async fn first_caller_wins_second_bails() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let run_id = setup_queued_run(db).await;

    let first = claim_run(db, run_id).await.unwrap();
    let second = claim_run(db, run_id).await.unwrap();

    assert!(first, "first claim must succeed");
    assert!(!second, "second claim must bail (already claimed)");

    // Run is now running with started_at populated.
    let after = job_runs::Entity::find_by_id(run_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "running");
    assert!(after.started_at.is_some());
}

#[tokio::test]
#[serial]
async fn parallel_claims_only_one_wins() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let run_id = setup_queued_run(db).await;

    // 8 parallel claim attempts simulate an aggressive race between the
    // post-job dispatch and the periodic sweep, plus sweeps from any
    // future external trigger.
    let claims = (0..8).map(|_| {
        let db = db.clone();
        tokio::spawn(async move { claim_run(&db, run_id).await.unwrap() })
    });

    let mut won = 0;
    for handle in claims {
        if handle.await.unwrap() {
            won += 1;
        }
    }

    assert_eq!(won, 1, "exactly one parallel claim must win");
}

#[tokio::test]
#[serial]
async fn already_running_run_is_not_reclaimable() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    let run_id = setup_queued_run(db).await;

    // First call wins, transitions to running.
    assert!(claim_run(db, run_id).await.unwrap());

    // Subsequent attempts (e.g. a stale sweep that read the row before
    // it was claimed) must not reset status or started_at.
    let before = job_runs::Entity::find_by_id(run_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let started_first = before.started_at;

    assert!(!claim_run(db, run_id).await.unwrap());

    let after = job_runs::Entity::find_by_id(run_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "running");
    assert_eq!(
        after.started_at, started_first,
        "started_at must not change"
    );
}

#[tokio::test]
#[serial]
async fn missing_run_id_returns_false_not_error() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;

    // Bogus id — the conditional UPDATE matches zero rows, which is the
    // same outcome as "another dispatcher claimed it". Caller bails;
    // no error surfaced.
    let claimed = claim_run(db, 999_999).await.unwrap();
    assert!(!claimed);
}
