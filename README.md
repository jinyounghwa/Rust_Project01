# Rust_Project01

<div align="center">
  <img src="./assets/network_monitor.png" alt="Network Monitor" width="600">
</div>

## Overview
This repository contains Rust programming projects, focusing on network monitoring and system utilities.

## Projects

### Network Monitor
A network monitoring tool that can detect network issues and perform automatic recovery actions. Features include:
- Real-time network status monitoring
- Automatic recovery actions
- GUI interface for easy management
- Windows service support

## Getting Started

### Prerequisites
- Rust and Cargo (latest stable version)
- Windows operating system

### Building the Projects
Navigate to the project directory and run:
```bash
cargo build --release
```

### Running the Network Monitor
Navigate to the network_monitor directory and run:
```bash
cargo run --release --features gui
```

### Standalone GUI Mode
For a standalone GUI application that doesn't require command-line arguments:

1. Build the GUI wrapper application:
```bash
cargo build --release --features gui --bin network_monitor_gui
```

2. Run the GUI application directly:
```bash
.\target\release\network_monitor_gui.exe
```

### Packaging the Application
To create a distributable package with standalone GUI mode:

1. Run the packaging script:
```bash
.\package.bat
```

2. This creates a `package` directory containing:
   - The main application executable
   - A standalone GUI executable
   - Configuration files
   - A script to create desktop shortcuts
   - Documentation

3. Distribute the `package` directory to users who can run `Network_Monitor_GUI.exe` directly.

## License
This project is licensed under the MIT License - see the LICENSE file for details.
