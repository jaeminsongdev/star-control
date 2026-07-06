use crate::json_fields::{
    optional_integer, optional_string, required_array, required_integer, required_string,
};
use crate::SentinelError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLines {
    pub task_id: String,
    pub files: Vec<ChangedFile>,
}

impl ChangedLines {
    pub fn from_value(value: &Value) -> Result<Self, SentinelError> {
        let files = required_array(value, "files", "ChangedLines")?
            .iter()
            .enumerate()
            .map(|(index, file)| {
                ChangedFile::from_value(file, &format!("ChangedLines.files[{}]", index))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            task_id: required_string(value, "task_id", "ChangedLines")?,
            files,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub change_type: String,
    pub old_path: Option<String>,
    pub hunks: Vec<ChangedHunk>,
}

impl ChangedFile {
    fn from_value(value: &Value, artifact: &str) -> Result<Self, SentinelError> {
        let hunks = required_array(value, "hunks", artifact)?
            .iter()
            .enumerate()
            .map(|(index, hunk)| {
                ChangedHunk::from_value(hunk, &format!("{}.hunks[{}]", artifact, index))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            path: required_string(value, "path", artifact)?,
            change_type: required_string(value, "change_type", artifact)?,
            old_path: optional_string(value, "old_path", artifact)?,
            hunks,
        })
    }

    pub(crate) fn changed_paths(&self) -> Vec<&str> {
        let mut paths = vec![self.path.as_str()];
        if let Some(old_path) = self.old_path.as_deref() {
            if old_path != self.path {
                paths.push(old_path);
            }
        }
        paths
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedHunk {
    pub old_start: i64,
    pub old_lines: i64,
    pub new_start: i64,
    pub new_lines: i64,
    pub lines: Vec<ChangedLine>,
}

impl ChangedHunk {
    fn from_value(value: &Value, artifact: &str) -> Result<Self, SentinelError> {
        let lines = required_array(value, "lines", artifact)?
            .iter()
            .enumerate()
            .map(|(index, line)| {
                ChangedLine::from_value(line, &format!("{}.lines[{}]", artifact, index))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            old_start: required_integer(value, "old_start", artifact)?,
            old_lines: required_integer(value, "old_lines", artifact)?,
            new_start: required_integer(value, "new_start", artifact)?,
            new_lines: required_integer(value, "new_lines", artifact)?,
            lines,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLine {
    pub kind: String,
    pub old_line: Option<i64>,
    pub new_line: Option<i64>,
    pub content: String,
}

impl ChangedLine {
    fn from_value(value: &Value, artifact: &str) -> Result<Self, SentinelError> {
        Ok(Self {
            kind: required_string(value, "kind", artifact)?,
            old_line: optional_integer(value, "old_line", artifact)?,
            new_line: optional_integer(value, "new_line", artifact)?,
            content: required_string(value, "content", artifact)?,
        })
    }
}
