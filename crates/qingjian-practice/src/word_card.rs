//! 本机词卡查询结果。
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PracticeWordCard {
    pub word: String,

    pub meaning: Option<String>,

    pub pronunciation: Option<String>,

    pub hint: Option<String>,
}
