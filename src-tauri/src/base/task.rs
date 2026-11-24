//! 下载任务：状态快照 + 定时把进度推给前端。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::base::net::Progress;
use crate::base::util::now_ms;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub id: u64,
    pub title: String,
    pub subtitle: String,
    pub target: String,
    /// running / done / failed / cancelled
    pub state: String,
    pub stage: String,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub speed: f64,
    pub sources: HashMap<String, u64>,
    pub error: Option<String>,
    pub note: Option<String>,
    pub log: Vec<String>,
    pub started: u64,
    pub finished: Option<u64>,
    pub result_path: Option<String>,
}

pub struct TaskEntry {
    pub snap: Mutex<TaskSnapshot>,
    pub prog: Arc<Progress>,
}

impl TaskEntry {
    pub fn stage(&self, s: impl Into<String>) {
        let s = s.into();
        let mut snap = self.snap.lock().unwrap();
        snap.log.push(s.clone());
        if snap.log.len() > 60 {
            snap.log.remove(0);
        }
        snap.stage = s;
    }
    pub fn log(&self, s: impl Into<String>) {
        let mut snap = self.snap.lock().unwrap();
        snap.log.push(s.into());
        if snap.log.len() > 60 {
            snap.log.remove(0);
        }
    }
    pub fn note(&self, s: impl Into<String>) {
        self.snap.lock().unwrap().note = Some(s.into());
    }
    /// 新一批文件开始前把计数清零，进度条按阶段走
    pub fn reset_counters(&self) {
        let p = &self.prog;
        for c in [&p.files_done, &p.files_total, &p.bytes_done, &p.bytes_total] {
            c.store(0, Ordering::Relaxed);
        }
    }
    pub fn snapshot(&self) -> TaskSnapshot {
        let mut s = self.snap.lock().unwrap().clone();
        s.files_done = self.prog.files_done.load(Ordering::Relaxed);
        s.files_total = self.prog.files_total.load(Ordering::Relaxed);
        s.bytes_done = self.prog.bytes_done.load(Ordering::Relaxed);
        s.bytes_total = self.prog.bytes_total.load(Ordering::Relaxed);
        s.sources = self.prog.source_bytes.lock().unwrap().clone();
        s
    }
}

#[derive(Default)]
pub struct Tasks {
    next: AtomicU64,
    list: Mutex<Vec<Arc<TaskEntry>>>,
}

impl Tasks {
    pub fn create(&self, title: String, subtitle: String, target: String) -> Arc<TaskEntry> {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let entry = Arc::new(TaskEntry {
            snap: Mutex::new(TaskSnapshot {
                id,
                title,
                subtitle,
                target,
                state: "running".into(),
                stage: "准备中".into(),
                started: now_ms(),
                ..Default::default()
            }),
            prog: Arc::new(Progress::default()),
        });
        self.list.lock().unwrap().push(entry.clone());
        entry
    }

    pub fn get(&self, id: u64) -> Option<Arc<TaskEntry>> {
        self.list.lock().unwrap().iter().find(|t| t.snap.lock().unwrap().id == id).cloned()
    }

    pub fn all(&self) -> Vec<TaskSnapshot> {
        self.list.lock().unwrap().iter().map(|t| t.snapshot()).collect()
    }

    pub fn clear_finished(&self) {
        self.list.lock().unwrap().retain(|t| t.snap.lock().unwrap().state == "running");
    }
}

/// 任务运行期间每 300ms 推一次进度；速度用最近一段时间的字节差算
pub fn spawn_ticker(app: AppHandle, task: Arc<TaskEntry>) {
    tauri::async_runtime::spawn(async move {
        let mut last_bytes = 0u64;
        let mut last_t = std::time::Instant::now();
        let mut smoothed = 0f64;
        loop {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let bytes = task.prog.bytes_done.load(Ordering::Relaxed);
            let dt = last_t.elapsed().as_secs_f64().max(0.001);
            let inst = bytes.saturating_sub(last_bytes) as f64 / dt;
            smoothed = if smoothed == 0.0 { inst } else { smoothed * 0.7 + inst * 0.3 };
            last_bytes = bytes;
            last_t = std::time::Instant::now();
            let mut snap = task.snapshot();
            snap.speed = smoothed;
            task.snap.lock().unwrap().speed = smoothed;
            let running = snap.state == "running";
            let _ = app.emit("task-update", &snap);
            if !running {
                break;
            }
        }
    });
}

pub fn finish(app: &AppHandle, task: &TaskEntry, result: anyhow::Result<Option<String>>) {
    {
        let mut snap = task.snap.lock().unwrap();
        snap.finished = Some(now_ms());
        snap.speed = 0.0;
        match result {
            Ok(path) => {
                snap.state = "done".into();
                snap.stage = "完成".into();
                snap.result_path = path;
            }
            Err(e) => {
                let msg = format!("{:#}", e);
                if task.prog.cancelled() {
                    snap.state = "cancelled".into();
                    snap.stage = "已取消".into();
                } else {
                    snap.state = "failed".into();
                    snap.stage = "失败".into();
                    snap.error = Some(msg);
                }
            }
        }
    }
    let _ = app.emit("task-update", &task.snapshot());
}
