// file: app/src/state.rs
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use kryvora_core::SanitizationOperationId;
use kryvora_storage::{DeviceSafetyAssessment, TargetIdentity, TargetKind};

pub(crate) fn database_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("kryvora.db")
}

#[derive(Debug)]
pub struct AppState {
    db_path: PathBuf,
    sanitization_plans: Mutex<HashMap<String, PendingSanitizationPlan>>,
    drive_plans: Mutex<HashMap<String, PendingDrivePlan>>,
}

#[derive(Debug)]
pub struct PendingSanitizationPlan {
    pub operation_id: SanitizationOperationId,
    pub case_id: Option<kryvora_core::CaseId>,
    pub actor: Option<String>,
    pub target_path: PathBuf,
    pub target_kind: TargetKind,
    pub identity: TargetIdentity,
}

#[derive(Debug)]
pub struct PendingDrivePlan {
    pub plan_id: String,
    pub device_path: String,
    pub device_identity: String,
    pub method: String,
    pub scope: String,
    pub assessment: DeviceSafetyAssessment,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub case_id: Option<kryvora_core::CaseId>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DrivePlanRequest {
    pub device_path: String,
    pub device_identity: String,
    pub method: String,
    pub scope: String,
    pub assessment: DeviceSafetyAssessment,
    pub case_id: Option<kryvora_core::CaseId>,
    pub actor: Option<String>,
}

impl AppState {
    #[must_use]
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            sanitization_plans: Mutex::new(HashMap::new()),
            drive_plans: Mutex::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn current_db(&self) -> &std::path::Path {
        &self.db_path
    }

    #[must_use]
    pub fn reports_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("reports")
    }

    #[must_use]
    pub fn recovered_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("recovered")
    }

    pub fn issue_sanitization_plan(
        &self,
        target_path: PathBuf,
        target_kind: TargetKind,
        identity: TargetIdentity,
        operation_id: SanitizationOperationId,
        case_id: Option<kryvora_core::CaseId>,
        actor: Option<String>,
    ) -> kryvora_core::Result<String> {
        let mut plans = self
            .sanitization_plans
            .lock()
            .map_err(|_| kryvora_core::Error::Internal("sanitization plan lock poisoned".into()))?;
        if plans.len() >= 64 {
            return Err(kryvora_core::Error::UnsafeTarget(
                "too many pending target confirmations; retry inspection".into(),
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        plans.insert(
            id.clone(),
            PendingSanitizationPlan {
                operation_id,
                case_id,
                actor,
                target_path,
                target_kind,
                identity,
            },
        );
        Ok(id)
    }

    pub fn take_sanitization_plan(
        &self,
        id: &str,
    ) -> kryvora_core::Result<Option<PendingSanitizationPlan>> {
        let mut plans = self
            .sanitization_plans
            .lock()
            .map_err(|_| kryvora_core::Error::Internal("sanitization plan lock poisoned".into()))?;
        Ok(plans.remove(id))
    }

    pub fn issue_drive_plan(&self, request: DrivePlanRequest) -> kryvora_core::Result<String> {
        let mut plans = self
            .drive_plans
            .lock()
            .map_err(|_| kryvora_core::Error::Internal("drive plan lock poisoned".into()))?;
        if plans.len() >= 32 {
            return Err(kryvora_core::Error::UnsafeTarget(
                "too many pending drive plans; re-inspect the device".into(),
            ));
        }
        let plan_id = uuid::Uuid::new_v4().to_string();
        let created_at = SystemTime::now();
        plans.insert(
            plan_id.clone(),
            PendingDrivePlan {
                plan_id: plan_id.clone(),
                device_path: request.device_path,
                device_identity: request.device_identity,
                method: request.method,
                scope: request.scope,
                assessment: request.assessment,
                created_at,
                expires_at: created_at + Duration::from_secs(900),
                case_id: request.case_id,
                actor: request.actor,
            },
        );
        Ok(plan_id)
    }

    pub fn take_drive_plan(&self, id: &str) -> kryvora_core::Result<Option<PendingDrivePlan>> {
        let mut plans = self
            .drive_plans
            .lock()
            .map_err(|_| kryvora_core::Error::Internal("drive plan lock poisoned".into()))?;
        Ok(plans.remove(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_path_is_under_the_app_data_directory() {
        let app_data_dir = PathBuf::from("platform-data").join("com.kryvora.desktop");
        let state = AppState::new(database_path(&app_data_dir));

        assert_eq!(
            state.current_db(),
            app_data_dir.join("kryvora.db").as_path()
        );
    }

    #[test]
    fn reports_directory_is_under_the_app_data_directory() {
        let app_data_dir = PathBuf::from("platform-data").join("com.kryvora.desktop");
        let state = AppState::new(database_path(&app_data_dir));

        assert_eq!(state.reports_dir(), app_data_dir.join("reports"));
    }

    #[test]
    fn recovered_directory_is_under_the_app_data_directory() {
        let app_data_dir = PathBuf::from("platform-data").join("com.kryvora.desktop");
        let state = AppState::new(database_path(&app_data_dir));

        assert_eq!(state.recovered_dir(), app_data_dir.join("recovered"));
    }

    #[test]
    fn sanitization_plan_can_only_be_consumed_once() {
        let target = std::env::temp_dir().join(format!(
            "kryvora-plan-{}.bin",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&target, b"disposable").unwrap();
        let identity = kryvora_storage::TargetIdentity::capture(
            &target,
            kryvora_storage::TargetKind::File,
        )
        .unwrap();
        let state = AppState::new(target.with_extension("db"));
        let operation_id = kryvora_core::SanitizationOperationId::new();
        let plan_id = state
            .issue_sanitization_plan(
                target.clone(),
                kryvora_storage::TargetKind::File,
                identity,
                operation_id,
                None,
                None,
            )
            .unwrap();

        assert!(state.take_sanitization_plan(&plan_id).unwrap().is_some());
        assert!(state.take_sanitization_plan(&plan_id).unwrap().is_none());
        std::fs::remove_file(target).unwrap();
    }
}
