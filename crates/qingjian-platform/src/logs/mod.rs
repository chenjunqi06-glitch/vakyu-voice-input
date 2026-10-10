//! 各进程日志的公共部分：按天分文件的命名与清理（[`daily`]）、密钥掩码（[`secrets`]）。
//! 日志目录本身在 [`crate::dirs::log_dir`]。

pub mod daily;
pub mod secrets;
