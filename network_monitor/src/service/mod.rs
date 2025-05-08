use crate::config::{self, Config};
use crate::monitor;
use log::{error, info};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use tokio::runtime::Runtime;
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher, service_manager::{ServiceManager, ServiceManagerAccess},
};

const SERVICE_NAME: &str = "NetworkMonitorService";
const SERVICE_DISPLAY_NAME: &str = "Network Monitor Service";
const SERVICE_DESCRIPTION: &str = "로컬 네트워크 장애 감지 및 자동 복구 서비스";

// Windows 서비스 정의
define_windows_service!(ffi_service_main, service_main);

// 서비스 설치 함수
pub fn install_service(config_path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 현재 실행 파일 경로 가져오기
    let exe_path = std::env::current_exe()?;
    let config_path = PathBuf::from(config_path).canonicalize()?;
    
    // 서비스 관리자 열기
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CREATE_SERVICE,
    )?;
    
    // 서비스 생성
    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path,
        launch_arguments: vec![
            OsString::from("service"),
            OsString::from("--config"),
            OsString::from(config_path.to_string_lossy().to_string()),
        ],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    
    let service = manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    
    // 서비스 설명 설정
    service.set_description(SERVICE_DESCRIPTION)?;
    
    info!("서비스가 성공적으로 설치되었습니다.");
    Ok(())
}

// 서비스 제거 함수
pub fn uninstall_service() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 서비스 관리자 열기
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT,
    )?;
    
    // 서비스 열기
    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::DELETE | ServiceAccess::STOP | ServiceAccess::QUERY_STATUS,
    )?;
    
    // 서비스가 실행 중이면 중지
    let service_status = service.query_status()?;
    if service_status.current_state != ServiceState::Stopped {
        service.stop()?;
        
        // 서비스가 중지될 때까지 대기
        let mut status = service_status;
        while status.current_state != ServiceState::Stopped {
            std::thread::sleep(Duration::from_secs(1));
            status = service.query_status()?;
        }
    }
    
    // 서비스 삭제
    service.delete()?;
    
    info!("서비스가 성공적으로 제거되었습니다.");
    Ok(())
}

// 서비스 실행 함수
pub async fn run_service(_config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 서비스 디스패처 실행
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

// 서비스 메인 함수
fn service_main(arguments: Vec<OsString>) {
    // 서비스 이벤트 핸들러 등록
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                info!("서비스 중지 요청 수신");
                shutdown_tx.send(()).unwrap_or_else(|e| {
                    error!("서비스 중지 신호 전송 실패: {}", e);
                });
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .unwrap_or_else(|e| {
            error!("서비스 컨트롤 핸들러 등록 실패: {}", e);
            panic!("서비스 컨트롤 핸들러 등록 실패: {}", e);
        });
    
    // 서비스 상태 업데이트: 시작 중
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(10),
            process_id: None,
        })
        .unwrap_or_else(|e| {
            error!("서비스 상태 업데이트 실패: {}", e);
        });
    
    // 설정 파일 경로 추출
    let config_path = arguments
        .iter()
        .position(|arg| arg == "--config")
        .and_then(|i| arguments.get(i + 1))
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.toml".to_string());
    
    // 설정 로드
    let config = match config::load_config(&config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("설정 파일 로드 실패: {}", e);
            
            // 서비스 상태 업데이트: 중지됨 (오류)
            status_handle
                .set_service_status(ServiceStatus {
                    service_type: ServiceType::OWN_PROCESS,
                    current_state: ServiceState::Stopped,
                    controls_accepted: ServiceControlAccept::empty(),
                    exit_code: ServiceExitCode::ServiceSpecific(1),
                    checkpoint: 0,
                    wait_hint: Duration::default(),
                    process_id: None,
                })
                .unwrap_or_else(|e| {
                    error!("서비스 상태 업데이트 실패: {}", e);
                });
            
            return;
        }
    };
    
    // 서비스 상태 업데이트: 실행 중
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .unwrap_or_else(|e| {
            error!("서비스 상태 업데이트 실패: {}", e);
        });
    
    // 비동기 런타임 생성 및 모니터링 시작
    let rt = Runtime::new().unwrap();
    let monitoring_handle = rt.spawn(async move {
        if let Err(e) = monitor::start_monitoring(config).await {
            error!("모니터링 오류: {}", e);
        }
    });
    
    // 종료 신호 대기
    shutdown_rx.recv().unwrap_or_else(|e| {
        error!("종료 신호 수신 실패: {}", e);
    });
    
    // 모니터링 작업 중단
    rt.block_on(async {
        monitoring_handle.abort();
        info!("모니터링 작업이 중단되었습니다.");
    });
    
    // 서비스 상태 업데이트: 중지됨
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .unwrap_or_else(|e| {
            error!("서비스 상태 업데이트 실패: {}", e);
        });
}
