use anyhow::{Context, Result};
use std::process::Command;
use sysinfo::{System};

#[derive(Debug)]
pub struct WindowInfo {
    pub window_id: String,
    pub title: String,
    pub class: String,
    pub pid: u32,
}

pub async fn switch_to_process_window(process_identifier: &str) -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();

    // Find the process
    let process = if let Ok(pid) = process_identifier.parse::<u32>() {
        system.process(sysinfo::Pid::from(pid as usize))
    } else {
        system
            .processes()
            .values()
            .find(|p| p.name().to_lowercase().contains(&process_identifier.to_lowercase()))
    };

    let process = process.context("Process not found")?;
    let pid = process.pid().as_u32();

    // Try to find window associated with this process
    let window = find_window_by_pid(pid)?;

    // Extract the program name for tool-goto-window
    let program_name = extract_program_name(&window)?;

    println!("Found window for process '{}' (PID: {})", process.name(), pid);
    println!("Window: {} - {}", window.class, window.title);
    println!("Switching to window using: tool-goto-window switch {}", program_name);

    // Use tool-goto-window to switch
    let output = Command::new("tool-goto-window")
        .arg("switch")
        .arg(&program_name)
        .output()
        .context("Failed to execute tool-goto-window")?;

    if output.status.success() {
        println!("Successfully switched to window");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Failed to switch window: {}", stderr);
    }

    Ok(())
}

pub fn find_window_by_pid(target_pid: u32) -> Result<WindowInfo> {
    // First try X11 approach
    if let Ok(window) = find_x11_window_by_pid(target_pid) {
        return Ok(window);
    }

    // If X11 fails, try alternative approaches
    find_window_by_process_name(target_pid)
}

fn find_x11_window_by_pid(target_pid: u32) -> Result<WindowInfo> {
    // Get all window IDs
    let output = Command::new("xdotool")
        .args(&["search", "--onlyvisible", "."])
        .output()
        .context("Failed to run xdotool search")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("xdotool search failed"));
    }

    let window_ids = String::from_utf8_lossy(&output.stdout);

    for window_id in window_ids.lines() {
        let window_id = window_id.trim();
        if window_id.is_empty() {
            continue;
        }

        // Get window PID
        if let Ok(window_pid) = get_window_pid(window_id) {
            if window_pid == target_pid {
                let title = get_window_title(window_id).unwrap_or_else(|_| "Unknown".to_string());
                let class = get_window_class(window_id).unwrap_or_else(|_| "Unknown".to_string());

                return Ok(WindowInfo {
                    window_id: window_id.to_string(),
                    title,
                    class,
                    pid: target_pid,
                });
            }
        }

        // Also check child processes
        if let Ok(child_pids) = get_process_children(target_pid) {
            if let Ok(window_pid) = get_window_pid(window_id) {
                if child_pids.contains(&window_pid) {
                    let title = get_window_title(window_id).unwrap_or_else(|_| "Unknown".to_string());
                    let class = get_window_class(window_id).unwrap_or_else(|_| "Unknown".to_string());

                    return Ok(WindowInfo {
                        window_id: window_id.to_string(),
                        title,
                        class,
                        pid: window_pid,
                    });
                }
            }
        }
    }

    Err(anyhow::anyhow!("No window found for PID {}", target_pid))
}

fn find_window_by_process_name(target_pid: u32) -> Result<WindowInfo> {
    let mut system = System::new_all();
    system.refresh_all();

    let process = system
        .process(sysinfo::Pid::from(target_pid as usize))
        .context("Process not found")?;

    let process_name = process.name();

    // Try to find window by process name using wmctrl
    let output = Command::new("wmctrl")
        .args(&["-l", "-p"])
        .output()
        .context("Failed to run wmctrl")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("wmctrl failed"));
    }

    let window_list = String::from_utf8_lossy(&output.stdout);

    for line in window_list.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            if let Ok(window_pid) = parts[2].parse::<u32>() {
                if window_pid == target_pid {
                    let window_id = parts[0];
                    let title = parts[4..].join(" ");

                    return Ok(WindowInfo {
                        window_id: window_id.to_string(),
                        title: title.clone(),
                        class: process_name.to_string(),
                        pid: target_pid,
                    });
                }
            }
        }
    }

    Err(anyhow::anyhow!("No window found for process {}", process_name))
}

fn get_window_pid(window_id: &str) -> Result<u32> {
    let output = Command::new("xdotool")
        .args(&["getwindowpid", window_id])
        .output()
        .context("Failed to get window PID")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to get PID for window {}", window_id));
    }

    let pid_str = String::from_utf8_lossy(&output.stdout);
    pid_str
        .trim()
        .parse::<u32>()
        .context("Failed to parse PID")
}

fn get_window_title(window_id: &str) -> Result<String> {
    let output = Command::new("xdotool")
        .args(&["getwindowname", window_id])
        .output()
        .context("Failed to get window title")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to get title for window {}", window_id));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_window_class(window_id: &str) -> Result<String> {
    let output = Command::new("xprop")
        .args(&["-id", window_id, "WM_CLASS"])
        .output()
        .context("Failed to get window class")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to get class for window {}", window_id));
    }

    let class_info = String::from_utf8_lossy(&output.stdout);
    // Parse WM_CLASS output: WM_CLASS(STRING) = "instance", "class"
    if let Some(start) = class_info.find('"') {
        if let Some(end) = class_info[start + 1..].find('"') {
            return Ok(class_info[start + 1..start + 1 + end].to_string());
        }
    }

    Ok("Unknown".to_string())
}

fn get_process_children(parent_pid: u32) -> Result<Vec<u32>> {
    let mut system = System::new_all();
    system.refresh_all();

    let children: Vec<u32> = system
        .processes()
        .values()
        .filter(|p| p.parent().map(|pp| pp.as_u32()) == Some(parent_pid))
        .map(|p| p.pid().as_u32())
        .collect();

    Ok(children)
}

fn extract_program_name(window: &WindowInfo) -> Result<String> {
    // Map common window classes/titles to tool-goto-window program names
    let program_name = match window.class.to_lowercase().as_str() {
        "code" | "vscode" => "code",
        "google-chrome" | "chrome" => "chrome",
        "firefox" => "firefox",
        "terminal" | "gnome-terminal" => "terminal",
        "nautilus" => "nautilus",
        _ => {
            // Check title for common patterns
            let title_lower = window.title.to_lowercase();
            if title_lower.contains("visual studio code") || title_lower.contains("vscode") {
                "code"
            } else if title_lower.contains("chrome") {
                "chrome"
            } else if title_lower.contains("firefox") {
                "firefox"
            } else if title_lower.contains("terminal") {
                "terminal"
            } else {
                // Use the class name as fallback
                &window.class.to_lowercase()
            }
        }
    };

    Ok(program_name.to_string())
}

pub fn list_all_windows() -> Result<Vec<WindowInfo>> {
    let mut windows = Vec::new();

    // Try X11 first
    if let Ok(x11_windows) = list_x11_windows() {
        windows.extend(x11_windows);
    }

    // If no X11 windows found, try wmctrl
    if windows.is_empty() {
        if let Ok(wmctrl_windows) = list_wmctrl_windows() {
            windows.extend(wmctrl_windows);
        }
    }

    Ok(windows)
}

fn list_x11_windows() -> Result<Vec<WindowInfo>> {
    let output = Command::new("xdotool")
        .args(&["search", "--onlyvisible", "."])
        .output()
        .context("Failed to run xdotool search")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("xdotool search failed"));
    }

    let window_ids = String::from_utf8_lossy(&output.stdout);
    let mut windows = Vec::new();

    for window_id in window_ids.lines() {
        let window_id = window_id.trim();
        if window_id.is_empty() {
            continue;
        }

        if let Ok(pid) = get_window_pid(window_id) {
            let title = get_window_title(window_id).unwrap_or_else(|_| "Unknown".to_string());
            let class = get_window_class(window_id).unwrap_or_else(|_| "Unknown".to_string());

            windows.push(WindowInfo {
                window_id: window_id.to_string(),
                title,
                class,
                pid,
            });
        }
    }

    Ok(windows)
}

fn list_wmctrl_windows() -> Result<Vec<WindowInfo>> {
    let output = Command::new("wmctrl")
        .args(&["-l", "-p"])
        .output()
        .context("Failed to run wmctrl")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("wmctrl failed"));
    }

    let window_list = String::from_utf8_lossy(&output.stdout);
    let mut windows = Vec::new();

    for line in window_list.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            if let Ok(pid) = parts[2].parse::<u32>() {
                let window_id = parts[0];
                let title = parts[4..].join(" ");

                windows.push(WindowInfo {
                    window_id: window_id.to_string(),
                    title: title.clone(),
                    class: "Unknown".to_string(),
                    pid,
                });
            }
        }
    }

    Ok(windows)
}