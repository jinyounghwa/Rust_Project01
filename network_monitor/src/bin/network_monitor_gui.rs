#![windows_subsystem = "windows"]

use std::process::Command;
use std::env;
use std::os::windows::process::CommandExt;

fn main() {
    // 현재 실행 파일의 경로 가져오기
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| env::current_dir().unwrap());
    
    // network_monitor.exe 경로 구성
    let exe_path = exe_dir.join("network_monitor.exe");
    
    // GUI 모드로 실행 (CREATE_NO_WINDOW 플래그 사용)
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let status = Command::new(exe_path)
        .arg("gui")
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    
    match status {
        Ok(exit_status) => {
            if !exit_status.success() {
                eprintln!("프로그램이 오류 코드 {}로 종료되었습니다", exit_status);
            }
        },
        Err(e) => {
            eprintln!("프로그램 실행 실패: {}", e);
        }
    }
}
