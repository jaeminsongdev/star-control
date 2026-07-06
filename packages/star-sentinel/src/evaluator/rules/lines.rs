use crate::changed_lines::{ChangedFile, ChangedLine};

pub(super) fn added_lines(file: &ChangedFile) -> impl Iterator<Item = &ChangedLine> {
    file.hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .filter(|line| line.kind == "added")
}

pub(super) fn changed_content_lines(file: &ChangedFile) -> impl Iterator<Item = &ChangedLine> {
    file.hunks
        .iter()
        .flat_map(|hunk| hunk.lines.iter())
        .filter(|line| line.kind == "added" || line.kind == "removed")
}
