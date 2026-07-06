use crate::JobSpec;

pub(super) fn normalized(job: &JobSpec) -> String {
    let constraints = job.user_constraints().join(" ");
    format!("{} {}", job.request_text(), constraints).to_lowercase()
}
