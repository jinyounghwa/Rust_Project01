use crate::config::Config;
use crate::network;
use crate::utils::logging;
use log::{error, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

static MONITORING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 네트워크 상태 확인 함수
/// 스레드 간 안전한 에러 타입을 사용합니다.
pub async fn check_status(config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("네트워크 상태 확인 시작");
    
    for target in &config.targets {
        let result = network::ping_host(&target.address, config.get_target_timeout(target)).await;
        match result {
            Ok(rtt) => {
                info!("대상 '{}' ({}) 응답 시간: {}ms", target.name, target.address, rtt.as_millis());
            }
            Err(e) => {
                warn!("대상 '{}' ({}) 응답 없음: {}", target.name, target.address, e);
            }
        }
        
        // 포트가 지정된 경우 포트 연결 테스트
        if let Some(port) = target.port {
            let result = network::check_port(&target.address, port, config.get_target_timeout(target)).await;
            match result {
                Ok(_) => {
                    info!("대상 '{}' ({}:{}) 포트 연결 성공", target.name, target.address, port);
                }
                Err(e) => {
                    warn!("대상 '{}' ({}:{}) 포트 연결 실패: {}", target.name, target.address, port, e);
                }
            }
        }
    }
    
    info!("네트워크 상태 확인 완료");
    Ok(())
}

/// 네트워크 모니터링 시작 함수
/// 스레드 간 안전한 에러 타입을 사용합니다.
pub async fn start_monitoring(config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 이미 모니터링 중인지 확인
    if MONITORING_ACTIVE.swap(true, Ordering::SeqCst) {
        warn!("이미 모니터링이 실행 중입니다");
        return Ok(());
    }
    
    info!("네트워크 모니터링 시작");
    
    // 로그 파일 설정
    if let Some(log_file) = &config.log_file {
        logging::setup_file_logger(log_file)?;
    }
    
    // Ctrl+C 핸들러 설정
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        info!("Ctrl+C 신호 감지, 모니터링 종료 중...");
        r.store(false, Ordering::SeqCst);
    })?;
    
    // 모니터링 루프
    let interval = Duration::from_secs(config.check_interval_sec);
    let config = Arc::new(config);
    
    while running.load(Ordering::SeqCst) {
        let mut all_targets_failed = true;
        
        // 모든 대상 확인
        for target in &config.targets {
            let retry_count = config.get_target_retry_count(target);
            let mut success = false;
            
            // 재시도 로직
            for attempt in 1..=retry_count {
                match network::ping_host(&target.address, config.get_target_timeout(target)).await {
                    Ok(rtt) => {
                        if attempt > 1 {
                            info!("대상 '{}' ({}) 재시도 #{} 성공, 응답 시간: {}ms", 
                                 target.name, target.address, attempt, rtt.as_millis());
                        } else {
                            info!("대상 '{}' ({}) 응답 시간: {}ms", 
                                 target.name, target.address, rtt.as_millis());
                        }
                        success = true;
                        break;
                    }
                    Err(e) => {
                        if attempt == retry_count {
                            error!("대상 '{}' ({}) 모든 재시도 실패: {}", 
                                  target.name, target.address, e);
                        } else {
                            warn!("대상 '{}' ({}) 재시도 #{} 실패: {}", 
                                 target.name, target.address, attempt, e);
                            time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            }
            
            if success {
                all_targets_failed = false;
            }
        }
        
        // 모든 대상이 실패했을 경우 복구 작업 수행
        if all_targets_failed && !config.recovery_actions.is_empty() {
            error!("모든 네트워크 대상 연결 실패, 복구 작업 시작");
            perform_recovery_actions(&config).await?;
        }
        
        // 다음 체크까지 대기
        time::sleep(interval).await;
    }
    
    // 모니터링 종료
    MONITORING_ACTIVE.store(false, Ordering::SeqCst);
    info!("네트워크 모니터링 종료");
    
    Ok(())
}

/// 복구 작업 수행 함수
async fn perform_recovery_actions(config: &Arc<Config>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for action in &config.recovery_actions {
        info!("복구 작업 '{}' 실행 중", action.name);
        
        match network::execute_command(&action.command).await {
            Ok(output) => {
                info!("복구 작업 '{}' 성공: {}", action.name, output);
                
                // 대기 시간이 지정된 경우 대기
                if let Some(wait_ms) = action.wait_after_ms {
                    info!("복구 작업 후 {}ms 대기 중", wait_ms);
                    time::sleep(Duration::from_millis(wait_ms)).await;
                }
                
                // 복구 후 네트워크 상태 다시 확인
                let recovered = check_recovery_success(config).await;
                if recovered {
                    info!("네트워크 연결이 복구되었습니다");
                    
                    // 알림 기능이 활성화된 경우 알림 전송
                    if config.notification_enabled {
                        if let Some(cmd) = &config.notification_command {
                            match network::execute_command(cmd).await {
                                Ok(_) => info!("복구 알림 전송 성공"),
                                Err(e) => warn!("복구 알림 전송 실패: {}", e),
                            }
                        }
                    }
                    
                    return Ok(());
                }
            }
            Err(e) => {
                error!("복구 작업 '{}' 실패: {}", action.name, e);
            }
        }
    }
    
    error!("모든 복구 작업이 실패했습니다");
    Ok(())
}

/// 복구 성공 여부 확인 함수
async fn check_recovery_success(config: &Arc<Config>) -> bool {
    // 기본 대상으로 ping 테스트
    match network::ping_host(&config.default_target, Duration::from_millis(config.ping_timeout_ms)).await {
        Ok(_) => true,
        Err(_) => false,
    }
}
