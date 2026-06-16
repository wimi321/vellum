use model_adapters::{
    MockStoryModel, ModelProviderProfile, ProviderTestResult, profile_from_json, profile_to_json,
    validate_profile,
};
use std::sync::Arc;
use story_harness_core::{PlaythroughState, StoryHarness};
use story_store::{BookManifest, BookSource, ImportJob, PlayerAction, PlayerIdentity, StoryStore};
use tauri::{Manager, State};

struct AppState {
    harness: StoryHarness,
}

#[tauri::command]
async fn configure_provider(
    state: State<'_, AppState>,
    mut profile: ModelProviderProfile,
) -> Result<ModelProviderProfile, String> {
    // V1 stores provider metadata locally and intentionally does not persist API keys until the
    // platform secure-storage bridge is enabled for both desktop and Android.
    profile.api_key = profile
        .api_key
        .as_ref()
        .filter(|key| !key.trim().is_empty())
        .map(|_| "__local_secret_not_persisted__".to_string());
    let json = profile_to_json(&profile).map_err(to_string)?;
    state
        .harness
        .store()
        .save_provider_profile_json(&profile.id, &json)
        .map_err(to_string)?;
    Ok(profile)
}

#[tauri::command]
async fn test_provider(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProviderTestResult, String> {
    let json = state
        .harness
        .store()
        .provider_profile_json(&profile_id)
        .map_err(to_string)?;
    let profile = profile_from_json(&json).map_err(to_string)?;
    Ok(validate_profile(&profile))
}

#[tauri::command]
async fn import_book(state: State<'_, AppState>, source: BookSource) -> Result<ImportJob, String> {
    let store = state.harness.store().clone();
    let job = store.start_import_book().map_err(to_string)?;
    let job_id = job.id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = store.run_import_job(&job_id, source);
    });
    Ok(job)
}

#[tauri::command]
async fn get_import_status(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<ImportJob, String> {
    state
        .harness
        .store()
        .import_status(&job_id)
        .map_err(to_string)
}

#[tauri::command]
async fn list_books(state: State<'_, AppState>) -> Result<Vec<BookManifest>, String> {
    state.harness.store().list_books().map_err(to_string)
}

#[tauri::command]
async fn start_playthrough(
    state: State<'_, AppState>,
    book_id: String,
    identity: PlayerIdentity,
) -> Result<PlaythroughState, String> {
    state
        .harness
        .start_playthrough(book_id, identity)
        .await
        .map_err(to_string)
}

#[tauri::command]
async fn resume_playthrough(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<PlaythroughState, String> {
    state
        .harness
        .resume_playthrough(session_id)
        .map_err(to_string)
}

#[tauri::command]
async fn send_player_action(
    state: State<'_, AppState>,
    session_id: String,
    action: PlayerAction,
) -> Result<PlaythroughState, String> {
    state
        .harness
        .send_player_action(session_id, action)
        .await
        .map_err(to_string)
}

#[tauri::command]
async fn rollback_turn(
    state: State<'_, AppState>,
    session_id: String,
    turn_id: String,
) -> Result<PlaythroughState, String> {
    state
        .harness
        .rollback_turn(session_id, turn_id)
        .map_err(to_string)
}

#[tauri::command]
async fn get_playthrough_state(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<PlaythroughState, String> {
    state
        .harness
        .resume_playthrough(session_id)
        .map_err(to_string)
}

#[tauri::command]
async fn get_evidence(
    state: State<'_, AppState>,
    turn_id: String,
) -> Result<Vec<story_store::EvidenceSpan>, String> {
    state
        .harness
        .store()
        .get_evidence(&turn_id)
        .map_err(to_string)
}

#[tauri::command]
async fn get_harness_trace(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<story_store::HarnessEvent>, String> {
    state
        .harness
        .store()
        .get_trace(&session_id)
        .map_err(to_string)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let store = StoryStore::open(data_dir)?;
            let harness = StoryHarness::new(store, Arc::new(MockStoryModel));
            app.manage(AppState { harness });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            configure_provider,
            test_provider,
            import_book,
            get_import_status,
            list_books,
            start_playthrough,
            resume_playthrough,
            send_player_action,
            rollback_turn,
            get_playthrough_state,
            get_evidence,
            get_harness_trace
        ])
        .run(tauri::generate_context!())
        .expect("error while running story harness app");
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}
