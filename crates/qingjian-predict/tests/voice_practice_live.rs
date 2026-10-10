//! 显式启用才调用真实云服务；只使用固定测试句，凭据仅从环境变量读取。

use std::time::{Duration, Instant};

use qingjian_predict::{PredictConfig, VoicePracticeTranslator};

#[test]
#[ignore = "requires QINGJIAN_VOICE_PRACTICE_LIVE=1 and cloud credentials in environment"]
fn fixed_chinese_sentence_translates_without_continuation() {
    assert_eq!(
        std::env::var("QINGJIAN_VOICE_PRACTICE_LIVE").as_deref(),
        Ok("1")
    );
    let defaults = PredictConfig::default();
    let config = PredictConfig {
        base_url: std::env::var("QINGJIAN_VOICE_PRACTICE_BASE_URL").unwrap_or(defaults.base_url),
        model: std::env::var("QINGJIAN_VOICE_PRACTICE_MODEL").unwrap_or(defaults.model),
        ..PredictConfig::default()
    };
    let mut translator = VoicePracticeTranslator::new(&config).unwrap();
    translator
        .request(1, "我今天要去吃饭，明天下午三点去拿快递。")
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(reply) = translator.try_recv() {
            let english = reply.result.unwrap();
            println!("fixed_sample_translation={english}");
            assert!(english.split_whitespace().count() >= 8);
            assert!(english.to_ascii_lowercase().contains("tomorrow"));
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(125),
            "translation did not finish within timeout"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
