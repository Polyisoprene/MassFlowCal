//! 应用配置：从环境变量加载，支持自定义数据库路径
use serde::Deserialize;

/// 应用配置结构体
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_db_path")]
    pub db_path: String,

    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

fn default_port() -> u16 { 3000 }
fn default_db_path() -> String { "data/massflowcal.db".to_string() }
fn default_data_dir() -> String { "data".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            port: default_port(),
            db_path: default_db_path(),
            data_dir: default_data_dir(),
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000),
            db_path: std::env::var("DB_PATH")
                .unwrap_or_else(|_| default_db_path()),
            data_dir: std::env::var("DATA_DIR")
                .unwrap_or_else(|_| default_data_dir()),
        }
    }
}
