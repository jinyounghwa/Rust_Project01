use log::{LevelFilter, SetLoggerError};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

// 디버그 모드 상태를 저장하는 전역 변수
static DEBUG_MODE: AtomicBool = AtomicBool::new(false);
static LOGGER_INIT: Once = Once::new();

pub mod logging {
    use super::*;
    use env_logger::Builder;
    use log::LevelFilter;
    use std::io::Write;

    /// 파일 로거를 설정합니다.
    pub fn setup_file_logger(log_file: &str) -> Result<(), SetLoggerError> {
        LOGGER_INIT.call_once(|| {
            let level = if DEBUG_MODE.load(Ordering::Relaxed) {
                LevelFilter::Debug
            } else {
                LevelFilter::Info
            };

            let file = match File::create(log_file) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("로그 파일 생성 실패: {}", e);
                    return;
                }
            };

            let mut builder = Builder::new();
            builder
                .format(|buf, record| {
                    writeln!(
                        buf,
                        "{} [{}] - {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        record.args()
                    )
                })
                .filter(None, level)
                .target(env_logger::Target::Pipe(Box::new(file)))
                .init();
        });

        Ok(())
    }

    /// 콘솔 로거를 설정합니다.
    pub fn setup_console_logger() -> Result<(), SetLoggerError> {
        LOGGER_INIT.call_once(|| {
            let level = if DEBUG_MODE.load(Ordering::Relaxed) {
                LevelFilter::Debug
            } else {
                LevelFilter::Info
            };

            let mut builder = Builder::new();
            builder
                .format(|buf, record| {
                    writeln!(
                        buf,
                        "{} [{}] - {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        record.args()
                    )
                })
                .filter(None, level)
                .init();
        });

        Ok(())
    }
}

/// 디버그 모드를 설정합니다.
pub fn set_debug_mode(enabled: bool) {
    DEBUG_MODE.store(enabled, Ordering::Relaxed);
}

/// 디버그 모드 상태를 반환합니다.
pub fn is_debug_mode() -> bool {
    DEBUG_MODE.load(Ordering::Relaxed)
}

/// 파일이 존재하는지 확인합니다.
pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists()
}

/// 현재 실행 파일의 경로를 반환합니다.
pub fn get_executable_path() -> Result<std::path::PathBuf, std::io::Error> {
    std::env::current_exe()
}

/// 현재 실행 파일의 디렉토리 경로를 반환합니다.
pub fn get_executable_dir() -> Result<std::path::PathBuf, std::io::Error> {
    let exe_path = get_executable_path()?;
    let dir = exe_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "실행 파일의 디렉토리를 찾을 수 없습니다.",
        )
    })?;
    Ok(dir.to_path_buf())
}

/// 파일 경로가 절대 경로인지 확인합니다.
pub fn is_absolute_path<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().is_absolute()
}

/// 상대 경로를 절대 경로로 변환합니다.
pub fn to_absolute_path<P: AsRef<Path>>(path: P) -> Result<std::path::PathBuf, std::io::Error> {
    if is_absolute_path(&path) {
        return Ok(path.as_ref().to_path_buf());
    }

    let exe_dir = get_executable_dir()?;
    Ok(exe_dir.join(path))
}
