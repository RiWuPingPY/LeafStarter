//! 「任务」页：列出、取消、清理任务

use tauri::State;

use crate::base::task::TaskSnapshot;
use crate::state::AppState;

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, id: u64) {
    if let Some(t) = state.tasks.get(id) {
        t.prog.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>) -> Vec<TaskSnapshot> {
    state.tasks.all()
}

#[tauri::command]
pub fn clear_tasks(state: State<'_, AppState>) -> Vec<TaskSnapshot> {
    state.tasks.clear_finished();
    state.tasks.all()
}
