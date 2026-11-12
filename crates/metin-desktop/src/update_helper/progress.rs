use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallPhase {
    Installing,
    Driver,
    Finishing,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallProgress {
    pub schema: u32,
    pub session: uuid::Uuid,
    pub phase: InstallPhase,
    pub completed: u64,
    pub total: u64,
}

impl InstallProgress {
    pub(super) const MAX_RECORD_BYTES: usize = 4096;
    pub fn decode(bytes: &[u8], session: uuid::Uuid) -> Result<Self, String> {
        if bytes.len() > Self::MAX_RECORD_BYTES {
            return Err("Installer progress exceeded its size limit.".into());
        }
        let record: Self =
            serde_json::from_slice(bytes).map_err(|_| "Invalid installer progress.")?;
        if record.schema != 1
            || record.session != session
            || record.total == 0
            || record.completed > record.total
        {
            return Err("Installer progress does not belong to this update.".into());
        }
        Ok(record)
    }

    pub fn fraction(&self) -> Option<f32> {
        (self.phase == InstallPhase::Installing)
            .then_some((self.completed as f64 / self.total as f64).min(0.99) as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_rejects_foreign_sessions_oversized_invalid_and_false_completion() {
        let session = uuid::Uuid::new_v4();
        let valid = serde_json::to_vec(&InstallProgress {
            schema: 1,
            session,
            phase: InstallPhase::Installing,
            completed: 5,
            total: 10,
        })
        .unwrap();
        assert_eq!(
            InstallProgress::decode(&valid, session).unwrap().fraction(),
            Some(0.5)
        );
        assert!(InstallProgress::decode(&valid, uuid::Uuid::new_v4()).is_err());
        assert!(InstallProgress::decode(&vec![b' '; 4097], session).is_err());
        assert!(InstallProgress::decode(br#"{"schema":1}"#, session).is_err());
        for (schema, completed, total) in [(2, 0, 1), (1, 2, 1), (1, 0, 0)] {
            let invalid = serde_json::to_vec(&InstallProgress {
                schema,
                session,
                phase: InstallPhase::Installing,
                completed,
                total,
            })
            .unwrap();
            assert!(InstallProgress::decode(&invalid, session).is_err());
        }
        let complete = InstallProgress {
            schema: 1,
            session,
            phase: InstallPhase::Installing,
            completed: 10,
            total: 10,
        };
        assert_eq!(
            complete.fraction(),
            Some(0.99),
            "File extraction is not verified update success"
        );
        for phase in [InstallPhase::Driver, InstallPhase::Finishing] {
            assert_eq!(
                InstallProgress {
                    phase,
                    ..complete.clone()
                }
                .fraction(),
                None
            );
        }
    }
}
