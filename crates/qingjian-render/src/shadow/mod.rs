//! 圆角面板阴影：三遍盒式模糊近似高斯，在线程内复用遮罩和工作区。

mod blur;
#[cfg(test)]
mod blur_tests;
mod key;
mod parameters;
mod scratch;
#[cfg(test)]
mod tests;

pub use parameters::Shadow;
