use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use tauri::Emitter;
use tauri::Manager;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 返回工具目录下“后端可执行文件”的标准文件名。
///
/// 命名约定：
/// - Windows: nscb_rust.exe
/// - macOS/Linux: nscb_rust（无后缀）
fn backend_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "nscb_rust.exe"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "nscb_rust"
    }
}

fn app_root_dir() -> Result<std::path::PathBuf, String> {
    #[cfg(debug_assertions)]
    {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if let Some(parent) = manifest_dir.parent() {
            return Ok(parent.to_path_buf());
        }
        return Ok(manifest_dir);
    }

    #[cfg(not(debug_assertions))]
    {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Failed to resolve current executable path: {e}"))?;
        if let Some(parent) = exe.parent() {
            return Ok(parent.to_path_buf());
        }
        Err("Failed to resolve executable directory".to_string())
    }
}

fn app_tools_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let temp_dir = app
        .path()
        .temp_dir()
        .map_err(|e| format!("Failed to resolve temp dir: {e}"))?;
    let tools_dir = temp_dir.join("nscb-desktop-tools");
    std::fs::create_dir_all(&tools_dir)
        .map_err(|e| format!("Failed to create tools dir: {e}"))?;
    Ok(tools_dir)
}

/// 获取后端可执行文件路径（跨平台）。
fn backend_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let tools_dir = app_tools_dir(app)?;
    let path = tools_dir.join(backend_filename());
    if !path.exists() {
        return Err(format!(
            "{} not found at {}",
            backend_filename(),
            path.display()
        ));
    }
    Ok(path)
}

/// 可选：如果构建时在 bundle resources 中带了 backend/prod.keys，则首次启动自动复制到 tools_dir。
///
/// 资源目录约定（打包进 resources 后的相对路径）：
/// - keys/prod.keys
/// - backend/macos/nscb_rust
/// - backend/windows/nscb_rust.exe
fn ensure_bundled_resources(app: &tauri::AppHandle) -> Result<(), String> {
    let tools_dir = app_tools_dir(app)?;

    // 1) prod.keys
    let keys_dst = tools_dir.join("prod.keys");
    if !keys_dst.exists() {
        if let Ok(res_path) =
            app.path()
                .resolve("keys/prod.keys", tauri::path::BaseDirectory::Resource)
        {
            if res_path.exists() {
                std::fs::copy(&res_path, &keys_dst)
                    .map_err(|e| format!("Failed to install bundled prod.keys: {e}"))?;
            }
        }
    }

    // 2) backend binary
    let backend_dst = tools_dir.join(backend_filename());
    if !backend_dst.exists() {
        let rel = if cfg!(target_os = "windows") {
            "backend/windows/nscb_rust.exe"
        } else if cfg!(target_os = "macos") {
            "backend/macos/nscb_rust"
        } else if cfg!(target_os = "linux") {
            "backend/linux/nscb_rust"
        } else {
            ""
        };

        if !rel.is_empty() {
            if let Ok(res_path) =
                app.path()
                    .resolve(rel, tauri::path::BaseDirectory::Resource)
            {
                if res_path.exists() {
                    std::fs::copy(&res_path, &backend_dst)
                        .map_err(|e| format!("Failed to install bundled backend: {e}"))?;

                    // 确保 unix 可执行位
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = std::fs::metadata(&backend_dst)
                            .map_err(|e| format!("Failed to read backend metadata: {e}"))?
                            .permissions();
                        perms.set_mode(perms.mode() | 0o111);
                        std::fs::set_permissions(&backend_dst, perms)
                            .map_err(|e| format!("Failed to chmod +x backend: {e}"))?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn running_pid() -> &'static Mutex<Option<u32>> {
    static PID: OnceLock<Mutex<Option<u32>>> = OnceLock::new();
    PID.get_or_init(|| Mutex::new(None))
}

#[derive(Serialize, Clone)]
struct StdoutEvent {
    op: String,
    line: String,
}

#[derive(Serialize, Clone)]
struct StderrEvent {
    op: String,
    chunk: String,
}

#[derive(Serialize, Clone)]
struct DoneEvent {
    op: String,
    code: i32,
}

/// A 方案：由后端提供平台信息，避免前端依赖 @tauri-apps/api/os
#[tauri::command]
fn get_platform() -> String {
    if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        "unknown".to_string()
    }
}

#[tauri::command]
fn import_keys(app: tauri::AppHandle, src_path: String) -> Result<(), String> {
    let tools_dir = app_tools_dir(&app)?;

    let dst_prod = tools_dir.join("prod.keys");
    std::fs::copy(&src_path, &dst_prod).map_err(|e| format!("Failed to copy prod.keys: {e}"))?;
    Ok(())
}

#[tauri::command]
fn get_tools_dir(app: tauri::AppHandle) -> Result<String, String> {
    let tools_dir = app_tools_dir(&app)?;
    Ok(tools_dir.to_string_lossy().into_owned())
}

#[tauri::command]
fn has_keys(app: tauri::AppHandle) -> Result<bool, String> {
    let tools_dir = app_tools_dir(&app)?;
    Ok(tools_dir.join("prod.keys").exists() || tools_dir.join("keys.txt").exists())
}

#[tauri::command]
fn has_backend(app: tauri::AppHandle) -> Result<bool, String> {
    let tools_dir = app_tools_dir(&app)?;
    Ok(tools_dir.join(backend_filename()).exists())
}

#[tauri::command]
fn import_nscb_binary(app: tauri::AppHandle, src_path: String) -> Result<(), String> {
    let src = std::path::PathBuf::from(&src_path);
    if !src.exists() {
        return Err("Selected file does not exist".to_string());
    }

    let filename = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    #[cfg(target_os = "windows")]
    {
        if filename != "nscb_rust.exe" && filename != "nscb_rust-x86_64-pc-windows-msvc.exe" {
            return Err(
                "Please select nscb_rust.exe (or nscb_rust-x86_64-pc-windows-msvc.exe)"
                    .to_string(),
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if filename != "nscb_rust" {
            return Err(
                "Please select the unix executable named 'nscb_rust' (no .exe suffix)"
                    .to_string(),
            );
        }
    }

    let tools_dir = app_tools_dir(&app)?;
    let dst = tools_dir.join(backend_filename());
    std::fs::copy(&src, &dst).map_err(|e| format!("Failed to copy backend: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = std::fs::metadata(&dst)
            .map_err(|e| format!("Failed to read backend metadata: {e}"))?
            .permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&dst, perms)
            .map_err(|e| format!("Failed to chmod +x backend: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
fn run_nscb(app: tauri::AppHandle, operation: String, args: Vec<String>) -> Result<(), String> {
    {
        let mut lock = running_pid()
            .lock()
            .map_err(|_| "Failed to lock runner state".to_string())?;
        if lock.is_some() {
            return Err("A process is already running".to_string());
        }

        let exe_path = backend_path(&app)?;
        let work_dir = app_root_dir()?;

        let mut cmd = Command::new(exe_path);
        cmd.args(args)
            .current_dir(work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start {}: {e}", backend_filename()))?;

        *lock = Some(child.id());

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to capture stderr".to_string())?;

        let app_for_out = app.clone();
        let op_for_out = operation.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let _ = app_for_out.emit(
                                "nscb-stdout",
                                StdoutEvent {
                                    op: op_for_out.clone(),
                                    line: trimmed.to_string(),
                                },
                            );
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let app_for_err = app.clone();
        let op_for_err = operation.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buf = [0_u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                        if !chunk.trim().is_empty() {
                            let _ = app_for_err.emit(
                                "nscb-stderr",
                                StderrEvent {
                                    op: op_for_err.clone(),
                                    chunk,
                                },
                            );
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let op_for_done = operation.clone();
        std::thread::spawn(move || {
            let code = match child.wait() {
                Ok(status) => status.code().unwrap_or(-1),
                Err(_) => -1,
            };
            if let Ok(mut pid_lock) = running_pid().lock() {
                *pid_lock = None;
            }
            let _ = app.emit(
                "nscb-done",
                DoneEvent {
                    op: op_for_done,
                    code,
                },
            );
        });
    }

    Ok(())
}

#[tauri::command]
fn get_backend_version(app: tauri::AppHandle) -> Result<String, String> {
    let tools_dir = app_tools_dir(&app)?;
    let version_file = tools_dir.join("version.txt");
    if version_file.exists() {
        std::fs::read_to_string(&version_file)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Failed to read version: {e}"))
    } else {
        Ok(String::new())
    }
}

#[tauri::command]
fn save_backend_version(app: tauri::AppHandle, version: String) -> Result<(), String> {
    let tools_dir = app_tools_dir(&app)?;
    std::fs::write(tools_dir.join("version.txt"), version.as_bytes())
        .map_err(|e| format!("Failed to save version: {e}"))
}

#[tauri::command]
fn download_backend(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let tools_dir = app_tools_dir(&app)?;
    let dst = tools_dir.join(backend_filename());

    let response = reqwest::blocking::get(&url).map_err(|e| format!("Download failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Download returned HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response body: {e}"))?;
    std::fs::write(&dst, &bytes).map_err(|e| format!("Failed to save backend: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut perms = std::fs::metadata(&dst)
            .map_err(|e| format!("Failed to read backend metadata: {e}"))?
            .permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&dst, perms)
            .map_err(|e| format!("Failed to chmod +x backend: {e}"))?;
    }

    Ok(())
}

#[tauri::command]
fn cancel_nscb() -> Result<(), String> {
    let pid_opt = {
        let mut lock = running_pid()
            .lock()
            .map_err(|_| "Failed to lock runner state".to_string())?;
        lock.take()
    };

    if let Some(pid) = pid_opt {
        #[cfg(target_os = "windows")]
        {
            let mut cmd = Command::new("taskkill");
            cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            cmd.creation_flags(CREATE_NO_WINDOW);
            let status = cmd
                .status()
                .map_err(|e| format!("Failed to stop process: {e}"))?;
            if !status.success() {
                return Err("Failed to stop running process".to_string());
            }
        }

        #[cfg(unix)]
        {
            let status = Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|e| format!("Failed to stop process: {e}"))?;

            if !status.success() {
                return Err("Failed to stop running process".to_string());
            }
        }
    }

    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 关键：启动时尝试从 bundle resources 自动安装（有则复制，无则跳过）
            ensure_bundled_resources(app.handle())?;
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            import_keys,
            import_nscb_binary,
            get_tools_dir,
            has_keys,
            has_backend,
            get_backend_version,
            save_backend_version,
            download_backend,
            run_nscb,
            cancel_nscb,
            get_platform
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
