//! 界面语言（中文 / English）。整个程序共用一个当前语言，由前端切换。

use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

static LANG: AtomicU8 = AtomicU8::new(0); // 0 = 中文, 1 = English

pub fn set(lang: &str) {
    LANG.store(if lang.starts_with("en") { 1 } else { 0 }, Relaxed);
}

pub fn is_en() -> bool {
    LANG.load(Relaxed) == 1
}

/// 按当前语言选一句
pub fn tr(zh: &str, en: &str) -> String {
    if is_en() { en } else { zh }.to_string()
}

/// 规则库里的双语文字；没有英文时退回中文
#[derive(Debug, Clone, Default)]
pub struct Text {
    pub zh: String,
    pub en: String,
}

impl Text {
    pub fn new(zh: &str, en: &str) -> Text {
        Text { zh: zh.to_string(), en: en.to_string() }
    }

    pub fn get(&self) -> &str {
        if is_en() && !self.en.is_empty() { &self.en } else { &self.zh }
    }
}
