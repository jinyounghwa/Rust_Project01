use std::process::Command;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;
use winping::{Buffer, Pinger};
use anyhow::{Result, anyhow};
use std::net::IpAddr;
use std::str::FromStr;

/// 호스트에 ICMP 핑 테스트를 수행합니다.
pub async fn ping_host(host: &str, timeout_duration: Duration) -> Result<Duration> {
    // 호스트 문자열을 IpAddr로 변환
    let ip_addr = IpAddr::from_str(host).map_err(|e| anyhow!("IP 주소 변환 실패: {}", e))?;
    
    // Windows용 ping 구현
    let pinger = Pinger::new().map_err(|e| anyhow!("Pinger 생성 실패: {}", e))?;
    let mut buffer = Buffer::new();
    
    let start = Instant::now();
    let _result = pinger.send(ip_addr, &mut buffer).map_err(|e| anyhow!("Ping 전송 실패: {}", e))?;
    
    // ping이 성공하면 elapsed 시간을 반환
    Ok(start.elapsed())
}

/// 지정된 호스트와 포트에 TCP 연결을 시도합니다.
pub async fn check_port(host: &str, port: u16, timeout_duration: Duration) -> Result<()> {
    let addr = format!("{}:{}", host, port);
    match timeout(timeout_duration, TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(anyhow!("포트 연결 실패: {}", e)),
        Err(_) => Err(anyhow!("포트 연결 시간 초과")),
    }
}

/// 네트워크 연결 테스트를 수행합니다.
pub async fn test_connection(host: &str) -> Result<()> {
    // 기본 타임아웃 설정
    let timeout_duration = Duration::from_secs(5);
    
    // ICMP 핑 테스트
    match ping_host(host, timeout_duration).await {
        Ok(rtt) => println!("ICMP 핑 성공: {}ms", rtt.as_millis()),
        Err(e) => println!("ICMP 핑 실패: {}", e),
    }
    
    // 일반적인 포트 테스트
    let common_ports = [80, 443, 8080];
    for port in common_ports {
        match check_port(host, port, timeout_duration).await {
            Ok(_) => println!("포트 {} 연결 성공", port),
            Err(e) => println!("포트 {} 연결 실패: {}", port, e),
        }
    }
    
    // 네트워크 인터페이스 정보 출력
    match get_network_interfaces() {
        Ok(output) => println!("네트워크 인터페이스 정보:\n{}", output),
        Err(e) => println!("네트워크 인터페이스 정보 가져오기 실패: {}", e),
    }
    
    Ok(())
}

/// 시스템 명령어를 실행하고 결과를 반환합니다.
pub async fn execute_command(cmd: &str) -> Result<String> {
    let output = TokioCommand::new("powershell")
        .args(["-Command", cmd])
        .output()
        .await
        .map_err(|e| anyhow!("명령어 실행 실패: {}", e))?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow!("명령어 실행 오류: {}", stderr))
    }
}

/// 네트워크 인터페이스 정보를 가져옵니다.
pub fn get_network_interfaces() -> Result<String> {
    let output = Command::new("powershell")
        .args(["-Command", "Get-NetAdapter | Format-Table -AutoSize"])
        .output()
        .map_err(|e| anyhow!("네트워크 인터페이스 정보 가져오기 실패: {}", e))?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow!("네트워크 인터페이스 정보 가져오기 오류: {}", stderr))
    }
}

/// 네트워크 인터페이스를 재시작합니다.
pub async fn restart_network_interface(interface_name: &str) -> Result<()> {
    let cmd = format!("Restart-NetAdapter -Name '{}' -Confirm:$false", interface_name);
    
    match execute_command(&cmd).await {
        Ok(_) => {
            println!("네트워크 인터페이스 '{}' 재시작 성공", interface_name);
            Ok(())
        }
        Err(e) => {
            println!("네트워크 인터페이스 '{}' 재시작 실패: {}", interface_name, e);
            Err(e)
        }
    }
}

/// DNS 캐시를 초기화합니다.
pub async fn flush_dns() -> Result<()> {
    match execute_command("Clear-DnsClientCache").await {
        Ok(_) => {
            println!("DNS 캐시 초기화 성공");
            Ok(())
        }
        Err(e) => {
            println!("DNS 캐시 초기화 실패: {}", e);
            Err(e)
        }
    }
}

/// IP 설정을 갱신합니다.
pub async fn renew_ip() -> Result<()> {
    match execute_command("ipconfig /renew").await {
        Ok(_) => {
            println!("IP 설정 갱신 성공");
            Ok(())
        }
        Err(e) => {
            println!("IP 설정 갱신 실패: {}", e);
            Err(e)
        }
    }
}
