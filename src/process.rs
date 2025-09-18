use anyhow::{Context, Result};
use procfs::process::Process;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use sysinfo::{System};
use tabled::{Table, Tabled, settings::{Width, object::Columns}};
use terminal_size::{Width as TermWidth, terminal_size};

#[derive(Tabled)]
pub struct ProcessInfo {
    #[tabled(rename = "PID")]
    pub pid: u32,

    #[tabled(rename = "Name")]
    pub name: String,

    #[tabled(rename = "Memory (MB)")]
    pub memory_mb: f64,

    #[tabled(rename = "CPU %")]
    pub cpu_percent: f32,

    #[tabled(rename = "Working Dir")]
    pub working_dir: String,

    #[tabled(rename = "Command")]
    pub command: String,
}

#[derive(Debug)]
pub struct DetailedProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_mb: f64,
    pub cpu_percent: f32,
    pub working_dir: Option<PathBuf>,
    pub command: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub parent_pid: Option<u32>,
    pub window_id: Option<String>,
    pub window_title: Option<String>,
}

pub async fn list_processes(
    name_filter: Option<&str>,
    high_memory: bool,
    sort_memory: bool,
    top_memory: Option<usize>,
    top_cpu: Option<usize>,
) -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut processes: Vec<ProcessInfo> = Vec::new();

    for (pid, process) in system.processes() {
        let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;

        // Apply filters
        if let Some(filter) = name_filter {
            if !process.name().to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }

        if high_memory && memory_mb < 100.0 {
            continue;
        }

        let working_dir = get_process_working_dir(pid.as_u32()).unwrap_or_else(|_| "N/A".to_string());

        let command = process
            .cmd()
            .first()
            .map(|c| {
                if c.len() > 50 {
                    format!("{}...", &c[..47])
                } else {
                    c.clone()
                }
            })
            .unwrap_or_else(|| "N/A".to_string());

        processes.push(ProcessInfo {
            pid: pid.as_u32(),
            name: process.name().to_string(),
            memory_mb: (memory_mb * 100.0).round() / 100.0,
            cpu_percent: process.cpu_usage(),
            working_dir,
            command,
        });
    }

    // Handle sorting and top N filtering
    if let Some(n) = top_memory {
        processes.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap());
        processes.truncate(n);
        println!("Top {} processes by memory usage:", n);
    } else if let Some(n) = top_cpu {
        processes.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap());
        processes.truncate(n);
        println!("Top {} processes by CPU usage:", n);
    } else if sort_memory {
        processes.sort_by(|a, b| b.memory_mb.partial_cmp(&a.memory_mb).unwrap());
    }

    let mut table = Table::new(processes);

    // Apply terminal width constraints
    if let Some((TermWidth(width), _)) = terminal_size() {
        let width = width as usize;

        // Reserve space for borders and padding (roughly 20 chars for table structure)
        let content_width = width.saturating_sub(20);

        // Distribute width among columns based on priority
        // PID: 8, Name: 15, Memory: 12, CPU: 8, Working Dir: flexible, Command: flexible
        let fixed_width = 8 + 15 + 12 + 8; // 43 chars for fixed columns
        let remaining_width = content_width.saturating_sub(fixed_width);

        if remaining_width > 0 {
            let working_dir_width = std::cmp::min(remaining_width / 2, 40);
            let command_width = remaining_width.saturating_sub(working_dir_width);

            table
                .modify(Columns::single(4), Width::truncate(working_dir_width).suffix("..."))
                .modify(Columns::single(5), Width::truncate(command_width).suffix("..."));
        } else {
            // Terminal is very narrow, apply aggressive truncation
            table
                .modify(Columns::single(1), Width::truncate(10).suffix("..."))
                .modify(Columns::single(4), Width::truncate(15).suffix("..."))
                .modify(Columns::single(5), Width::truncate(20).suffix("..."));
        }
    }

    println!("{}", table);

    Ok(())
}

pub async fn show_process_info(process_identifier: &str) -> Result<()> {
    let mut system = System::new_all();
    system.refresh_all();

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

    let detailed_info = get_detailed_process_info(pid, process)?;

    let terminal_width = terminal_size().map(|(TermWidth(w), _)| w as usize).unwrap_or(80);
    let max_value_width = terminal_width.saturating_sub(25); // Reserve space for labels

    println!("Process Information:");
    println!("  PID: {}", detailed_info.pid);
    println!("  Name: {}", detailed_info.name);
    println!("  Memory: {:.2} MB", detailed_info.memory_mb);
    println!("  CPU: {:.1}%", detailed_info.cpu_percent);

    if let Some(wd) = &detailed_info.working_dir {
        let wd_str = wd.display().to_string();
        if wd_str.len() > max_value_width {
            println!("  Working Directory: {}...", &wd_str[..max_value_width.saturating_sub(3)]);
        } else {
            println!("  Working Directory: {}", wd_str);
        }
    }

    if let Some(ppid) = detailed_info.parent_pid {
        println!("  Parent PID: {}", ppid);
    }

    let command_str = detailed_info.command.join(" ");
    if command_str.len() > max_value_width {
        println!("  Command: {}...", &command_str[..max_value_width.saturating_sub(3)]);
    } else {
        println!("  Command: {}", command_str);
    }

    if let Some(window_id) = &detailed_info.window_id {
        if window_id.len() > max_value_width {
            println!("  Window ID: {}...", &window_id[..max_value_width.saturating_sub(3)]);
        } else {
            println!("  Window ID: {}", window_id);
        }
    }

    if let Some(window_title) = &detailed_info.window_title {
        if window_title.len() > max_value_width {
            println!("  Window Title: {}...", &window_title[..max_value_width.saturating_sub(3)]);
        } else {
            println!("  Window Title: {}", window_title);
        }
    }

    println!("\nEnvironment Variables (relevant):");
    for (key, value) in &detailed_info.env_vars {
        if is_relevant_env_var(key) {
            let env_max_width = terminal_width.saturating_sub(key.len() + 5); // key + ": " + "  "
            if value.len() > env_max_width {
                println!("  {}: {}...", key, &value[..env_max_width.saturating_sub(3)]);
            } else {
                println!("  {}: {}", key, value);
            }
        }
    }

    Ok(())
}

fn get_process_working_dir(pid: u32) -> Result<String> {
    let cwd_path = format!("/proc/{}/cwd", pid);
    let cwd = fs::read_link(&cwd_path)
        .with_context(|| format!("Failed to read working directory for PID {}", pid))?;

    Ok(cwd.to_string_lossy().to_string())
}

fn get_detailed_process_info(
    pid: u32,
    sysinfo_process: &sysinfo::Process,
) -> Result<DetailedProcessInfo> {
    let memory_mb = sysinfo_process.memory() as f64 / 1024.0 / 1024.0;

    let working_dir = get_process_working_dir(pid).ok().map(PathBuf::from);

    let mut env_vars = HashMap::new();
    let mut window_info = None;

    // Try to get detailed process info from procfs
    if let Ok(process) = Process::new(pid as i32) {
        // Get environment variables
        if let Ok(environ) = process.environ() {
            for (key, value) in environ {
                env_vars.insert(key.to_string_lossy().to_string(), value.to_string_lossy().to_string());
            }
        }

        // Get window information if available
        window_info = get_window_info_for_process(pid, &env_vars).ok();
    }

    Ok(DetailedProcessInfo {
        pid,
        name: sysinfo_process.name().to_string(),
        memory_mb: (memory_mb * 100.0).round() / 100.0,
        cpu_percent: sysinfo_process.cpu_usage(),
        working_dir,
        command: sysinfo_process.cmd().to_vec(),
        env_vars,
        parent_pid: sysinfo_process.parent().map(|p| p.as_u32()),
        window_id: window_info.as_ref().map(|(id, _)| id.clone()),
        window_title: window_info.map(|(_, title)| title),
    })
}

fn get_window_info_for_process(
    _pid: u32,
    env_vars: &HashMap<String, String>,
) -> Result<(String, String)> {
    // Check if process has DISPLAY variable (X11)
    if env_vars.contains_key("DISPLAY") {
        // We'll implement X11 window detection in the window module
        return Err(anyhow::anyhow!("Window detection not implemented yet"));
    }

    // Check if process has WAYLAND_DISPLAY (Wayland)
    if env_vars.contains_key("WAYLAND_DISPLAY") {
        return Err(anyhow::anyhow!("Wayland window detection not implemented yet"));
    }

    Err(anyhow::anyhow!("No display environment detected"))
}

fn is_relevant_env_var(key: &str) -> bool {
    matches!(
        key,
        "DISPLAY"
            | "WAYLAND_DISPLAY"
            | "PWD"
            | "TERM"
            | "TERM_PROGRAM"
            | "VSCODE_PID"
            | "VSCODE_IPC_HOOK_CLI"
            | "WINDOWID"
            | "XTERM_VERSION"
    )
}