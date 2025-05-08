mod config;
mod monitor;
mod network;
mod service;
mod utils;

#[cfg(feature = "gui")]
mod gui;

use clap::{Parser, Subcommand};
use log::{error, info};
use std::process;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 설정 파일 경로
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// 디버그 모드 활성화
    #[arg(short, long)]
    debug: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 서비스로 실행
    Service {
        /// 서비스 설치
        #[arg(short, long)]
        install: bool,

        /// 서비스 제거
        #[arg(short, long)]
        uninstall: bool,
    },
    /// 네트워크 상태 확인
    Status,
    /// 네트워크 연결 테스트
    Test {
        /// 테스트할 호스트 주소
        #[arg(short, long)]
        host: Option<String>,
    },
    /// GUI 모드로 실행
    #[cfg(feature = "gui")]
    Gui,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Ctrl+C 핸들러 설정
    let (tx, rx) = std::sync::mpsc::channel();
    ctrlc::set_handler(move || {
        println!("Ctrl+C 감지됨. 모니터링을 안전하게 종료합니다...");
        tx.send(()).expect("Ctrl+C 시그널 전송 실패");
    })
    .expect("Ctrl+C 핸들러 설정 실패");

    // 명령줄 인자 파싱
    let cli = Cli::parse();
    
    // 로깅 초기화 설정
    if cli.debug {
        utils::set_debug_mode(true);
    }
    
    // 명령줄에서 직접 실행할 때만 콘솔 로거 초기화 (서비스 모드가 아닐 때)
    if !matches!(cli.command, Some(Commands::Service { .. })) {
        match utils::logging::setup_console_logger() {
            Ok(_) => {},
            Err(e) => {
                eprintln!("로그 초기화 실패: {}", e);
                return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
            }
        }
    }
    
    // 설정 로드
    let config_path = cli.config;
    let config = match config::load_config(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("설정 파일 로드 실패: {}", e);
            process::exit(1);
        }
    };
    
    // 디버그 모드 설정
    if cli.debug {
        utils::set_debug_mode(true);
    }
    
    // 명령 처리
    match &cli.command {
        Some(Commands::Service { install, uninstall }) => {
            if *install {
                info!("서비스 설치 중...");
                match service::install_service(&config_path) {
                    Ok(_) => {},
                    Err(e) => {
                        error!("서비스 설치 실패: {}", e);
                        return Err(e);
                    }
                }
            } else if *uninstall {
                info!("서비스 제거 중...");
                match service::uninstall_service() {
                    Ok(_) => {},
                    Err(e) => {
                        error!("서비스 제거 실패: {}", e);
                        return Err(e);
                    }
                }
            } else {
                info!("서비스 모드로 실행 중...");
                match service::run_service(config).await {
                    Ok(_) => {},
                    Err(e) => {
                        error!("서비스 실행 실패: {}", e);
                        return Err(e);
                    }
                }
            }
        },
        Some(Commands::Status) => {
            info!("네트워크 상태 확인 중...");
            match monitor::check_status(&config).await {
                Ok(_) => {},
                Err(e) => {
                    error!("네트워크 상태 확인 실패: {}", e);
                    return Err(e);
                }
            }
        },
        Some(Commands::Test { host }) => {
            let target = host.clone().unwrap_or_else(|| config.default_target.clone());
            info!("네트워크 연결 테스트 중: {}", target);
            match network::test_connection(&target).await {
                Ok(_) => {},
                Err(e) => {
                    error!("네트워크 연결 테스트 실패: {}", e);
                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("네트워크 연결 테스트 실패: {}", e))) as Box<dyn std::error::Error + Send + Sync>);
                }
            }
        },
        #[cfg(feature = "gui")]
        Some(Commands::Gui) => {
            info!("GUI 모드로 실행 중...");
            match gui::run_gui(config_path) {
                Ok(_) => {},
                Err(e) => {
                    error!("GUI 실행 실패: {}", e);
                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("GUI 실행 실패: {}", e))) as Box<dyn std::error::Error + Send + Sync>);
                }
            }
        },
        None => {
            info!("모니터링 시작 중...");
            // 모니터링 시작 전에 Ctrl+C 수신 대기 스레드 시작
            let monitoring_handle = tokio::spawn(monitor::start_monitoring(config));
            
            // Ctrl+C 신호 대기
            rx.recv().unwrap_or(());
            info!("모니터링을 종료합니다.");
            
            // 모니터링 작업이 완료될 때까지 대기
            match tokio::time::timeout(std::time::Duration::from_secs(5), monitoring_handle).await {
                Ok(_) => info!("모니터링이 정상적으로 종료되었습니다."),
                Err(_) => info!("모니터링 종료 시간이 초과되었습니다. 강제 종료합니다.")
            }
        },
    }
    
    // 반환
    Ok(())
}
