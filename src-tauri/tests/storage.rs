use chrono::NaiveDate;
use std::fs;
use tempfile::tempdir;
use ticknest_lib::storage::{Store, StoreError};
use ticknest_lib::tasks::TaskKind;

#[test]
fn store_creates_schema_and_round_trips_tasks() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("todo.sqlite3");
    let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();

    {
        let mut store = Store::open(&path).unwrap();
        let task = store.create_task("写周报", TaskKind::Today, today).unwrap();
        store.set_task_completed(task.id, true, today).unwrap();
    }

    let mut store = Store::open(&path).unwrap();
    let tasks = store.list_tasks(today).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "写周报");
    assert_eq!(tasks[0].completed_on, Some(today));
}

#[test]
fn round_trips_note_after_reopen() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("todo.sqlite3");

    {
        let mut store = Store::open(&path).unwrap();
        store.save_note("周五前交表格\n记得带钥匙").unwrap();
    }

    let store = Store::open(&path).unwrap();
    assert_eq!(store.load_note().unwrap(), "周五前交表格\n记得带钥匙");
}

#[test]
fn refuses_corrupt_database_without_overwriting_it() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("todo.sqlite3");
    let original = b"not a SQLite database";
    fs::write(&path, original).unwrap();

    match Store::open(&path) {
        Err(StoreError::CorruptDatabase) => {}
        Err(error) => panic!("expected corrupt database error, got {error}"),
        Ok(_) => panic!("corrupt database unexpectedly opened"),
    }

    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn rejects_newer_schema_without_downgrading_it() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("todo.sqlite3");
    let newer_version = 99;
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "user_version", newer_version)
        .unwrap();
    drop(connection);

    match Store::open(&path) {
        Err(StoreError::UnsupportedSchema(version)) => assert_eq!(version, newer_version),
        Err(error) => panic!("expected unsupported schema error, got {error}"),
        Ok(_) => panic!("newer database schema unexpectedly opened"),
    }

    let connection = rusqlite::Connection::open(&path).unwrap();
    let actual_version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(actual_version, newer_version);
}

#[test]
fn rejects_stale_task_id_and_does_not_change_other_tasks() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("todo.sqlite3");
    let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
    let mut store = Store::open(&path).unwrap();
    let stale = store
        .create_task("已经删除", TaskKind::Today, today)
        .unwrap();
    let other = store
        .create_task("保留任务", TaskKind::Today, today)
        .unwrap();
    store.delete_task(stale.id).unwrap();

    assert!(matches!(
        store.set_task_completed(stale.id, true, today),
        Err(StoreError::NotFound)
    ));
    let tasks = store.list_tasks(today).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, other.id);
    assert_eq!(tasks[0].completed_on, None);
}

#[test]
fn rename_task_normalizes_title_and_preserves_it_after_invalid_input() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("todo.sqlite3");
    let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
    let mut store = Store::open(&path).unwrap();
    let task = store.create_task("旧标题", TaskKind::Today, today).unwrap();

    store.rename_task(task.id, "  新标题 ").unwrap();
    assert!(matches!(
        store.rename_task(task.id, " \t "),
        Err(StoreError::InvalidTask(_))
    ));
    assert_eq!(store.list_tasks(today).unwrap()[0].title, "新标题");
}

#[test]
fn list_tasks_carries_overdue_incomplete_items() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("todo.sqlite3");
    let yesterday = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
    let mut store = Store::open(&path).unwrap();
    let incomplete = store
        .create_task("未完成事项", TaskKind::Today, yesterday)
        .unwrap();
    let completed = store
        .create_task("已完成事项", TaskKind::Today, yesterday)
        .unwrap();
    store
        .set_task_completed(completed.id, true, yesterday)
        .unwrap();

    let tasks = store.list_tasks(today).unwrap();
    assert_eq!(tasks.len(), 2);
    let incomplete = tasks.iter().find(|task| task.id == incomplete.id).unwrap();
    assert_eq!(incomplete.due_date, Some(today));
    assert_eq!(incomplete.completed_on, None);
    let completed = tasks.iter().find(|task| task.id == completed.id).unwrap();
    assert_eq!(completed.due_date, Some(yesterday));
    assert_eq!(completed.completed_on, Some(yesterday));

    let tasks_again = store.list_tasks(today).unwrap();
    assert_eq!(tasks_again.len(), 2);
    assert_eq!(
        tasks_again
            .iter()
            .find(|task| task.id == incomplete.id)
            .unwrap()
            .due_date,
        Some(today)
    );
}
