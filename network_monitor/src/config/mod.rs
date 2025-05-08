use serde::{Deserialize, Serialize, de::Error as SerdeError};
use std::fs;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("설정 파일을 읽을 수 없음: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("설정 파일 파싱 오류: {0}")]
    ParseError(#[from] toml::de::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkTarget {
    pub name: String,
    pub address: String,
    pub port: Option<u16>,
    pub timeout_ms: Option<u64>,
    pub retry_count: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecoveryAction {
    pub name: String,
    pub command: String,
    pub wait_after_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub default_target: String,
    pub check_interval_sec: u64,
    pub ping_timeout_ms: u64,
    pub retry_count: u8,
    pub targets: Vec<NetworkTarget>,
    pub recovery_actions: Vec<RecoveryAction>,
    pub log_file: Option<String>,
    pub notification_enabled: bool,
    pub notification_command: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_target: "8.8.8.8".to_string(),
            check_interval_sec: 60,
            ping_timeout_ms: 1000,
            retry_count: 3,
            targets: vec![
                NetworkTarget {
                    name: "Google DNS".to_string(),
                    address: "8.8.8.8".to_string(),
                    port: None,
                    timeout_ms: Some(1000),
                    retry_count: Some(3),
                },
                NetworkTarget {
                    name: "Local Router".to_string(),
                    address: "192.168.1.1".to_string(),
                    port: None,
                    timeout_ms: Some(500),
                    retry_count: Some(2),
                },
            ],
            recovery_actions: vec![
                RecoveryAction {
                    name: "네트워크 어댑터 재시작".to_string(),
                    command: "powershell -Command \"Restart-NetAdapter -Name 'Ethernet' -Confirm:$false\"".to_string(),
                    wait_after_ms: Some(5000),
                },
            ],
            log_file: Some("network_monitor.log".to_string()),
            notification_enabled: true,
            notification_command: Some("powershell -Command \"[System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms'); [System.Windows.Forms.MessageBox]::Show('네트워크 연결이 복구되었습니다.', '네트워크 모니터', [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Information)\"".to_string()),
        }
    }
}

impl Config {
    pub fn get_target_timeout(&self, target: &NetworkTarget) -> Duration {
        Duration::from_millis(target.timeout_ms.unwrap_or(self.ping_timeout_ms))
    }

    pub fn get_target_retry_count(&self, target: &NetworkTarget) -> u8 {
        target.retry_count.unwrap_or(self.retry_count)
    }
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config, ConfigError> {
    // 설정 파일이 존재하지 않으면 기본 설정을 저장하고 반환
    if !path.as_ref().exists() {
        let default_config = Config::default();
        let toml_string = toml::to_string_pretty(&default_config)
            .map_err(|e| ConfigError::ParseError(toml::de::Error::custom(format!("{}", e))))?;
        fs::write(&path, toml_string)?;
        return Ok(default_config);
    }

    // 설정 파일 읽기
    let config_str = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&config_str)?;
    
    Ok(config)
}

pub fn save_config<P: AsRef<Path>>(config: &Config, path: P) -> Result<(), ConfigError> {
    let toml_string = toml::to_string_pretty(config)
        .map_err(|e| ConfigError::ParseError(toml::de::Error::custom(format!("{}", e))))?;
    fs::write(path, toml_string)?;
    Ok(())
}
