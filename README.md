# Top Helper

A Rust CLI tool to monitor system resources and track process contexts. This tool helps you understand what's using resources on your system and provides context about where processes are running.

## Features

- **Resource Monitoring**: View processes with memory usage, CPU usage, and working directories
- **Context Detection**: See where processes were executed (working directory, environment variables)
- **Window Integration**: Find and switch to GUI windows associated with processes
- **Filtering Options**: Filter by process name, memory usage, and sort options

## Installation

```bash
cargo install --path .
```

This installs the `top-helper` binary to `~/.cargo/bin/top-helper`.

## Usage

### List Processes

```bash
# List all processes with basic info
top-helper list

# Filter by process name
top-helper list -n chrome

# Show only high memory processes (>100MB)
top-helper list --high-memory

# Sort by memory usage (descending)
top-helper list --sort-memory

# Combine filters
top-helper list -n code --high-memory --sort-memory
```

### Get Detailed Process Information

```bash
# Get detailed info by process name
top-helper info claude

# Get detailed info by PID
top-helper info 12345
```

This shows:
- Memory usage and CPU usage
- Working directory where the process was started
- Parent process
- Full command line
- Relevant environment variables (DISPLAY, TERM, etc.)
- Window information (if available)

### Switch to Process Window

```bash
# Switch to window containing the process
top-helper switch chrome
top-helper switch 12345
```

This attempts to:
1. Find the window associated with the process
2. Determine the appropriate program name for `tool-goto-window`
3. Switch to that window using `tool-goto-window switch <program>`

## Example Output

### List High Memory Processes
```
+---------+-----------------+-------------+--------+--------------------------------------------+---------------------------+
| PID     | Name            | Memory (MB) | CPU %  | Working Dir                                | Command                   |
+---------+-----------------+-------------+--------+--------------------------------------------+---------------------------+
| 2899794 | File Watcher    | 574.33      | 0      | /home/user/projects/my-project             | claude                    |
| 2789109 | node            | 605.10      | 0      | /home/user/projects/web-app                | node                      |
| 1879008 | code            | 132.34      | 0      | /home/user/projects/my-app                 | /usr/share/code/code      |
+---------+-----------------+-------------+--------+--------------------------------------------+---------------------------+
```

### Detailed Process Info
```
Process Information:
  PID: 2899794
  Name: File Watcher
  Memory: 574.33 MB
  CPU: 0.0%
  Working Directory: /home/user/projects/my-project
  Parent PID: 2885643
  Command: claude

Environment Variables (relevant):
  DISPLAY: :1
  TERM: xterm-256color
  TERM_PROGRAM: vscode
  PWD: /home/user/projects/my-project
```

## Use Cases

1. **Resource Investigation**: "What's using 1.4GB of memory?"
   ```bash
   top-helper list --high-memory --sort-memory
   ```

2. **Context Discovery**: "Where was this process started?"
   ```bash
   top-helper info claude
   ```

3. **Window Management**: "Switch to the VSCode window running this process"
   ```bash
   top-helper switch code
   ```

4. **Project Tracking**: "What processes are running in my project directory?"
   ```bash
   top-helper list | grep /home/user/projects/my-project
   ```

## Dependencies

- `xdotool` - for X11 window detection and manipulation
- `wmctrl` - alternative window management (fallback)
- `tool-goto-window` - for actual window switching

## Technical Details

The tool uses:
- `sysinfo` crate for process information
- `procfs` for detailed process data (working directories, environment variables)
- X11 tools (`xdotool`, `wmctrl`) for window detection
- Integration with `tool-goto-window` for actual window switching

Window detection works by:
1. Finding all visible windows using `xdotool`
2. Matching window PIDs to target process PIDs
3. Checking child processes for window associations
4. Mapping window classes to `tool-goto-window` program names

## Contributing

The tool is designed to be extended with additional process context detection and window management features.