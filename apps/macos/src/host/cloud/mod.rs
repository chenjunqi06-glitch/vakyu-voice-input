//! 云端：测试连接的起停与轮询，联想的等待 / 取消 / 轮询与结果套用到当前页。

mod predict_monitor;
mod test_monitor;

pub(super) use predict_monitor::PredictMonitor;
pub(super) use test_monitor::CloudTestMonitor;

use qingjian_core::{Candidate, CandidateKind, CloudWord, Prediction};
use qingjian_predict::{ConnectionTest, PredictError};

use crate::host::Host;

impl Host {
    /// 「测试连接」：用当前配置发一条最小请求，结果回到按钮旁与窗口底部。
    pub(super) fn start_cloud_test(&mut self) {
        if self.cloud_test.is_some() {
            return;
        }
        let config = self.settings.config().predict.clone();
        match ConnectionTest::start(&config) {
            Ok(test) => {
                self.cloud_test = Some(test);
                self.cloud_test_monitor.start();
                self.preferences.set_cloud_test_status(
                    &format!("正在测试云服务连接：{} …", config.model),
                    true,
                );
            }
            Err(error) => {
                self.preferences.set_cloud_test_status(
                    &format!("云服务连接测试没能开始：{}", describe_predict_error(&error)),
                    false,
                );
            }
        }
    }

    /// 轮询定时器每 0.2 秒来一次：结果到了就显示并停表；等太久也停。
    pub fn poll_cloud_test(&mut self) {
        let Some(test) = &self.cloud_test else {
            self.cloud_test_monitor.stop();
            return;
        };
        let Some(outcome) = test.poll() else {
            if self.cloud_test_monitor.expired() {
                self.cloud_test = None;
                self.cloud_test_monitor.stop();
                self.preferences.set_cloud_test_status(
                    "云服务连接测试：等了 30 秒没有结果，请检查网络或代理",
                    false,
                );
            }
            return;
        };
        self.cloud_test = None;
        self.cloud_test_monitor.stop();
        match outcome {
            Ok(report) => self.preferences.set_cloud_test_status(
                &format!(
                    "云服务连接成功：{} 在 {} ms 内回复",
                    report.model,
                    report.elapsed.as_millis()
                ),
                false,
            ),
            Err(error) => self.preferences.set_cloud_test_status(
                &format!("云服务连接失败：{}", describe_predict_error(&error)),
                false,
            ),
        }
    }

    /// 已发出联想请求：清掉旧结果，开始轮询。
    pub fn await_prediction(&mut self) {
        self.sentence = None;
        self.monitor.start();
    }

    /// 作废联想：不再轮询，丢掉已到的整句。
    pub fn cancel_prediction(&mut self) {
        self.engine.cancel_prediction();
        self.monitor.stop();
        self.sentence = None;
    }

    /// 定时器回调：结果到了就画上去。云端词补进第一页末尾，整句挂在 preedit 右侧。
    pub fn poll_prediction(&mut self) {
        let Some(prediction) = self.engine.poll_prediction() else {
            if self.monitor.expired() {
                self.monitor.stop();
            }
            return;
        };
        self.monitor.stop();
        self.apply_prediction(prediction);
    }

    pub(super) fn apply_prediction(&mut self, prediction: Prediction) {
        // 翻译选中文字：译文作为唯一候选摆进窗口，等用户回车替换或 Esc 放弃
        if let Some(job) = self.translation.as_mut() {
            match prediction.sentence {
                Some(text) => {
                    job.result = Some(text.clone());
                    self.reset_session(None, vec![cloud_candidate(text)]);
                    self.render();
                }
                None => {
                    tracing::info!("云端没有给出译文");
                    self.end_translation();
                }
            }
            return;
        }
        if self.engine.composition().is_empty() {
            return;
        }
        // 云端词补进第一页末尾（前面的本地候选不动），整句挂在 preedit 右侧；云端词没有译文，先补上。
        // 用户已经翻到后面的页、或高亮已经移到会被挪走的那几格时不补：那几格正被他看着 / 要选
        let layout = &self.session.layout;
        let untouched = cloud_slots_untouched(
            self.session.page,
            self.session.highlighted,
            layout.local().is_empty(),
            layout.page_size(),
            layout.capacity(),
        );
        if untouched && !prediction.words.is_empty() {
            let mut words = qingjian_core::CandidateList {
                items: prediction
                    .words
                    .into_iter()
                    .map(CloudWord::into_candidate)
                    .collect(),
            };
            self.engine.annotate(&mut words);
            let filled = self.session.layout.set_cloud(words.items);
            tracing::debug!(filled, "云端词已补进候选");
        }
        self.sentence = prediction.sentence;
        self.render();
    }
}

/// 用系统的 `open` 打开文件或目录；输入法进程没有自己的文档窗口，交给访达 / 默认编辑器最省事。
/// 云端词到了还能不能补进第一页：用户还在第一页，且高亮没落在会被云端词顶掉的那几格上。
/// 没有本地候选（问字模式）时整页都是云端的，谈不上挪走谁，永远能补——
/// 之前没分这种情况，`page_size - capacity` 算出 0，问字的答案全被当成「会打扰用户」丢掉了。
pub(super) fn cloud_slots_untouched(
    page: usize,
    highlighted: usize,
    local_empty: bool,
    page_size: usize,
    capacity: usize,
) -> bool {
    page == 0 && (local_empty || highlighted < page_size.saturating_sub(capacity))
}

/// 连通性测试的错误换成给用户看的中文；接口返回的原话保留，方便对着服务商文档查。
pub(super) fn describe_predict_error(error: &PredictError) -> String {
    match error {
        PredictError::MissingApiKey(env) => {
            format!("没有 API 密钥。在上面填一个，或设置环境变量 {env}")
        }
        PredictError::Timeout(ms) => {
            format!("{ms} ms 内没有回复。检查网络或代理：输入法进程不读终端里的代理变量")
        }
        PredictError::EmptyReply => "接口通了但没有返回正文，检查模型名是否正确".to_owned(),
        PredictError::IncompleteReply => {
            "回复被截断，未得到完整正文，请增加输出额度后重试".to_owned()
        }
        PredictError::BudgetExhausted => {
            "接口通了但正文是空的：模型开着思考，把输出额度用光了".to_owned()
        }
        PredictError::Encode(error) => format!("请求编码失败：{error}"),
        PredictError::Runtime(error) => format!("起不了后台线程：{error}"),
        PredictError::Api(error) => format!("请求失败：{error}"),
    }
}

/// 翻译窗口里的一行：译文（或占位文字）当作云端来源的候选画出来。
pub(super) fn cloud_candidate(text: String) -> Candidate {
    Candidate {
        text,
        kind: CandidateKind::Cloud,
        syllables: Vec::new(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn question_answers_fill_the_page_even_though_every_slot_is_cloud() {
        // 问字：没有本地候选，capacity == page_size
        assert!(super::cloud_slots_untouched(0, 0, true, 9, 9));
        // 组句：高亮在前面的本地格上能补，高亮已在末尾两格（云端要占的位置）不补，翻页后不补
        assert!(super::cloud_slots_untouched(0, 0, false, 9, 2));
        assert!(!super::cloud_slots_untouched(0, 7, false, 9, 2));
        assert!(!super::cloud_slots_untouched(1, 0, false, 9, 2));
    }
}
