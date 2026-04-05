const VALID_SEVERITIES: &[&str] = &["low", "medium", "high", "critical"];
const VALID_STATUSES: &[&str] = &["new", "triaged", "investigating", "contained", "resolved", "closed"];
const VALID_RESOLUTIONS: &[&str] = &["true_positive", "false_positive", "benign", "informational", "other"];

pub fn validate_severity(s: &str) -> Result<(), &'static str> {
    if VALID_SEVERITIES.contains(&s) {
        Ok(())
    } else {
        Err("invalid severity")
    }
}

pub fn validate_status(s: &str) -> Result<(), &'static str> {
    if VALID_STATUSES.contains(&s) {
        Ok(())
    } else {
        Err("invalid status")
    }
}

pub fn validate_resolution(s: &str) -> Result<(), &'static str> {
    if VALID_RESOLUTIONS.contains(&s) {
        Ok(())
    } else {
        Err("invalid resolution")
    }
}
