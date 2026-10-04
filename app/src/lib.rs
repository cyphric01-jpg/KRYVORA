// file: app/src/lib.rs
//! KRYVORA Tauri backend.
//!
//! Thin wrappers over the workspace crate functions. Business logic
//! lives in the workspace crates; this module only marshals parameters
//! and results across the Tauri boundary.

mod commands;
mod error;
mod state;

pub use error::CommandError;
pub use state::AppState;

use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            app.manage(AppState::new(state::database_path(&app_data_dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::verify_chain,
            commands::list_devices,
            commands::inspect_drive_target,
            commands::plan_drive_sanitization,
            commands::inspect_sanitize_target,
            commands::cancel_sanitization_plan,
            commands::list_sanitization_operations,
            commands::list_cases,
            commands::create_case,
            commands::list_evidence_for_case,
            commands::register_evidence,
            commands::verify_evidence,
            commands::hash_file,
            commands::sanitize_file,
            commands::sanitize_folder,
            commands::carve_source,
            commands::list_recovery_results,
            commands::list_jobs,
            commands::list_audit_events,
            commands::list_reports,
            commands::generate_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KRYVORA");
}
