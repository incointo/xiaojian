use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskKind {
    Today,
    Daily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub kind: TaskKind,
    pub due_date: Option<NaiveDate>,
    pub completed_on: Option<NaiveDate>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskValidationError {
    Empty,
    TooLong,
}

pub fn normalize_title(title: &str) -> Result<String, TaskValidationError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(TaskValidationError::Empty);
    }
    if title.chars().count() > 200 {
        return Err(TaskValidationError::TooLong);
    }
    Ok(title.to_owned())
}

pub fn is_complete_today(task: &Task, today: NaiveDate) -> bool {
    task.completed_on == Some(today)
}

pub fn carry_forward(task: &mut Task, today: NaiveDate) -> bool {
    if task.kind != TaskKind::Today || task.completed_on.is_some() {
        return false;
    }

    match task.due_date {
        Some(due_date) if due_date < today => {
            task.due_date = Some(today);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        carry_forward, is_complete_today, normalize_title, Task, TaskKind, TaskValidationError,
    };
    use chrono::NaiveDate;

    #[test]
    fn trims_nonblank_title() {
        assert_eq!(normalize_title("  准备早餐 \n").unwrap(), "准备早餐");
    }

    #[test]
    fn rejects_blank_title() {
        assert_eq!(normalize_title(" \t\n"), Err(TaskValidationError::Empty));
    }

    #[test]
    fn rejects_title_over_200_unicode_characters() {
        let accepted = "字".repeat(200);
        let rejected = "字".repeat(201);

        assert_eq!(normalize_title(&accepted).unwrap(), accepted);
        assert_eq!(
            normalize_title(&rejected),
            Err(TaskValidationError::TooLong)
        );
    }

    #[test]
    fn daily_completion_is_scoped_to_local_date() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let tomorrow = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let task = Task {
            id: 1,
            title: "读书".to_owned(),
            kind: TaskKind::Daily,
            due_date: None,
            completed_on: Some(today),
            sort_order: 0,
        };

        assert!(is_complete_today(&task, today));
        assert!(!is_complete_today(&task, tomorrow));
    }

    #[test]
    fn carry_forward_moves_only_overdue_incomplete_today_tasks() {
        let yesterday = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 20).unwrap();
        let tomorrow = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let mut overdue = Task {
            id: 1,
            title: "交报告".to_owned(),
            kind: TaskKind::Today,
            due_date: Some(yesterday),
            completed_on: None,
            sort_order: 0,
        };

        assert!(carry_forward(&mut overdue, today));
        assert_eq!(overdue.due_date, Some(today));
        assert!(!carry_forward(&mut overdue, today));

        let mut completed = Task {
            completed_on: Some(yesterday),
            ..overdue.clone()
        };
        completed.due_date = Some(yesterday);
        assert!(!carry_forward(&mut completed, today));
        assert_eq!(completed.due_date, Some(yesterday));

        let mut daily = Task {
            kind: TaskKind::Daily,
            due_date: None,
            completed_on: None,
            ..overdue.clone()
        };
        assert!(!carry_forward(&mut daily, today));

        let mut future = Task {
            due_date: Some(tomorrow),
            completed_on: None,
            ..overdue
        };
        assert!(!carry_forward(&mut future, today));
        assert_eq!(future.due_date, Some(tomorrow));
    }
}
