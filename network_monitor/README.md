# 네트워크 모니터링 도구

로컬 네트워크 장애를 감지하고 자동으로 복구하는 Rust 기반 도구입니다.

## 기능

- 다양한 네트워크 대상에 대한 주기적인 연결 모니터링
- ICMP 핑 및 TCP 포트 연결 테스트
- 네트워크 장애 발생 시 자동 복구 작업 수행
- Windows 서비스로 실행 가능
- 상세한 로깅 및 알림 기능

## 설치 방법

### 요구 사항

- Rust 및 Cargo (최신 버전)
- Windows 운영 체제

### 빌드 방법

```powershell
cargo build --release
```

빌드된 실행 파일은 `target/release/network_monitor.exe`에 생성됩니다.

## 사용 방법

### 기본 실행

```powershell
.\network_monitor.exe
```

### 네트워크 상태 확인

```powershell
.\network_monitor.exe status
```

### 특정 호스트 연결 테스트

```powershell
.\network_monitor.exe test --host 8.8.8.8
```

### Windows 서비스로 설치

```powershell
.\network_monitor.exe service --install
```

### Windows 서비스 제거

```powershell
.\network_monitor.exe service --uninstall
```

### 디버그 모드로 실행

```powershell
.\network_monitor.exe --debug
```

### 사용자 지정 설정 파일 사용

```powershell
.\network_monitor.exe --config my_config.toml
```

## 설정 파일

프로그램은 첫 실행 시 기본 설정 파일(`config.toml`)을 생성합니다. 이 파일을 수정하여 모니터링 대상, 복구 작업 등을 사용자 지정할 수 있습니다.

### 설정 예시

```toml
default_target = "8.8.8.8"
check_interval_sec = 60
ping_timeout_ms = 1000
retry_count = 3
log_file = "network_monitor.log"
notification_enabled = true

[[targets]]
name = "Google DNS"
address = "8.8.8.8"
timeout_ms = 1000
retry_count = 3

[[targets]]
name = "Local Router"
address = "192.168.1.1"
timeout_ms = 500
retry_count = 2

[[recovery_actions]]
name = "네트워크 어댑터 재시작"
command = "powershell -Command \"Restart-NetAdapter -Name 'Ethernet' -Confirm:$false\""
wait_after_ms = 5000
```

## 라이선스

이 프로젝트는 MIT 라이선스 하에 배포됩니다.
