use chrono::Utc;

use crate::models::{AlertItem, AlertState};

pub(super) fn seed_alerts() -> Vec<AlertItem> {
    vec![
        AlertItem {
            id: "ALRT-1001".to_string(),
            title: "Multiple failed admin logins from rare geo".to_string(),
            severity: "high".to_string(),
            source: "Identity".to_string(),
            mitre_tactic: "TA0006 Credential Access".to_string(),
            fired_at: Utc::now().to_rfc3339(),
            state: AlertState::Firing,
        },
        AlertItem {
            id: "ALRT-1002".to_string(),
            title: "Suspicious lateral movement via SMB".to_string(),
            severity: "critical".to_string(),
            source: "Network".to_string(),
            mitre_tactic: "TA0008 Lateral Movement".to_string(),
            fired_at: Utc::now().to_rfc3339(),
            state: AlertState::Firing,
        },
        AlertItem {
            id: "ALRT-1003".to_string(),
            title: "EDR detected unsigned powershell execution".to_string(),
            severity: "medium".to_string(),
            source: "Endpoint".to_string(),
            mitre_tactic: "TA0005 Defense Evasion".to_string(),
            fired_at: Utc::now().to_rfc3339(),
            state: AlertState::Acknowledged,
        },
    ]
}
