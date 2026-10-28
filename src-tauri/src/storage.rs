use crate::tasks::{carry_forward, normalize_title, Task, TaskKind, TaskValidationError};
use chrono::NaiveDate;
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::fmt;
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug)]
pub enum StoreError {
    Sql(rusqlite::Error),
    InvalidTask(TaskValidationError),
    NotFound,
    CorruptDatabase,
    UnsupportedSchema(i64),
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(error) => write!(formatter, "本地数据库错误：{error}"),
            Self::InvalidTask(TaskValidationError::Empty) => {
                formatter.write_str("任务标题不能为空。")
            }
            Self::InvalidTask(TaskValidationError::TooLong) => {
                formatter.write_str("任务标题不能超过 200 个字符。")
            }
            Self::NotFound => formatter.write_str("任务不存在，可能已被删除。"),
            Self::CorruptDatabase => formatter.write_str("本地数据库损坏或结构无效。"),
            Self::UnsupportedSchema(version) => {
                write!(formatter, "本地数据库版本 {version} 暂不受支持。")
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut connection = Connection::open(path)?;
        let integrity: Result<String, _> =
            connection.query_row("PRAGMA quick_check", [], |row| row.get(0));
        if !matches!(integrity, Ok(result) if result == "ok") {
            return Err(StoreError::CorruptDatabase);
        }

        let schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| StoreError::CorruptDatabase)?;
        if schema_version > SCHEMA_VERSION {
            return Err(StoreError::UnsupportedSchema(schema_version));
        }

        if schema_version == 0 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE tasks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    kind TEXT NOT NULL CHECK (kind IN ('today', 'daily')),
                    due_date TEXT,
                    completed_on TEXT,
                    sort_order INTEGER NOT NULL,
                    CHECK ((kind = 'today' AND due_date IS NOT NULL) OR
                           (kind = 'daily' AND due_date IS NULL))
                );
                CREATE INDEX tasks_scope_order ON tasks(kind, due_date, sort_order, id);
                CREATE TABLE app_state (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    note TEXT NOT NULL DEFAULT ''
                );
                INSERT INTO app_state(id, note) VALUES (1, '');",
            )?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
        } else {
            let table_count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table' AND name IN ('tasks', 'app_state')",
                [],
                |row| row.get(0),
            )?;
            if table_count != 2 {
                return Err(StoreError::CorruptDatabase);
            }
        }

        Ok(Self { connection })
    }

    pub fn list_tasks(&mut self, today: NaiveDate) -> Result<Vec<Task>, StoreError> {
        let date = format_date(today);
        let transaction = self.connection.transaction()?;
        let overdue = {
            let mut statement = transaction.prepare(
                "SELECT id, title, kind, due_date, completed_on, sort_order
                 FROM tasks
                 WHERE kind = 'today' AND completed_on IS NULL AND due_date < ?1",
            )?;
            let rows = statement.query_map([&date], task_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        for mut task in overdue {
            if carry_forward(&mut task, today) {
                transaction.execute(
                    "UPDATE tasks SET due_date = ?1 WHERE id = ?2",
                    params![date, task.id],
                )?;
            }
        }

        let tasks = {
            let mut statement = transaction.prepare(
                "SELECT id, title, kind, due_date, completed_on, sort_order
                 FROM tasks
                 WHERE kind = 'daily'
                    OR (kind = 'today' AND (completed_on = ?1
                        OR (completed_on IS NULL AND due_date = ?1)))
                 ORDER BY CASE kind WHEN 'today' THEN 0 ELSE 1 END, sort_order, id",
            )?;
            let rows = statement.query_map([&date], task_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        transaction.commit()?;
        Ok(tasks)
    }

    pub fn create_task(
        &mut self,
        title: &str,
        kind: TaskKind,
        today: NaiveDate,
    ) -> Result<Task, StoreError> {
        let title = normalize_title(title).map_err(StoreError::InvalidTask)?;
        let kind_name = kind_name(kind);
        let transaction = self.connection.transaction()?;
        let order: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks WHERE kind = ?1",
            [kind_name],
            |row| row.get(0),
        )?;
        let due_date = match kind {
            TaskKind::Today => Some(today),
            TaskKind::Daily => None,
        };
        transaction.execute(
            "INSERT INTO tasks(title, kind, due_date, completed_on, sort_order)
             VALUES (?1, ?2, ?3, NULL, ?4)",
            params![title, kind_name, due_date.map(format_date), order],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.commit()?;

        Ok(Task {
            id,
            title,
            kind,
            due_date,
            completed_on: None,
            sort_order: order,
        })
    }

    pub fn set_task_completed(
        &mut self,
        id: i64,
        completed: bool,
        today: NaiveDate,
    ) -> Result<(), StoreError> {
        let completed_on = completed.then(|| format_date(today));
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE tasks SET completed_on = ?1 WHERE id = ?2",
            params![completed_on, id],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn rename_task(&mut self, id: i64, title: &str) -> Result<(), StoreError> {
        let title = normalize_title(title).map_err(StoreError::InvalidTask)?;
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE tasks SET title = ?1 WHERE id = ?2",
            params![title, id],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_task(&mut self, id: i64) -> Result<(), StoreError> {
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn load_note(&self) -> Result<String, StoreError> {
        self.connection
            .query_row("SELECT note FROM app_state WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or(StoreError::CorruptDatabase)
    }

    pub fn save_note(&mut self, note: &str) -> Result<(), StoreError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO app_state(id, note) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET note = excluded.note",
            [note],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn kind_name(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Today => "today",
        TaskKind::Daily => "daily",
    }
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn parse_date(value: Option<String>, column: usize) -> rusqlite::Result<Option<NaiveDate>> {
    value
        .map(|value| {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
            })
        })
        .transpose()
}

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<Task> {
    let kind_value: String = row.get(2)?;
    let kind = match kind_value.as_str() {
        "today" => TaskKind::Today,
        "daily" => TaskKind::Daily,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                2,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "unknown task kind",
                )),
            ));
        }
    };

    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        kind,
        due_date: parse_date(row.get(3)?, 3)?,
        completed_on: parse_date(row.get(4)?, 4)?,
        sort_order: row.get(5)?,
    })
}
