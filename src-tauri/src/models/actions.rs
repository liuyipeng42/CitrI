use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ActionItem {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub enabled: bool,
    pub built_in: bool,
    pub prompt: String,
}

impl Default for ActionItem {
    fn default() -> Self {
        Self {
            id: "custom".into(),
            name: "自定义".into(),
            icon: "sparkles".into(),
            enabled: true,
            built_in: false,
            prompt: String::new(),
        }
    }
}

pub fn default_actions() -> Vec<ActionItem> {
    vec![
        action("copy", "复制", "copy", true, ""),
        action("search", "搜索", "search", true, ""),
        action(
            "translate",
            "翻译",
            "languages",
            true,
            r#"你是一个专业划词翻译助手。请先判断输入文本的语种与类型（单词 / 短语、句子或段落），再严格按以下规则输出。

## 翻译规则

1. **外文单词（如英文单词或常见缩写）**
   * 给出常用音标（如适用，使用斜杠 `/.../`）与词性、简体中文常见释义；
   * 如有多个常见词性或义项，分条列出最常用的即可，不堆砌生僻义；
   * 仅当极易混淆或一词多义差异较大时，补充一句简短用法区分；除非理解必须，不附例句。

2. **中文词语**
   * 给出地道常用的英文对应词及词性；多个常用译法时分条列出，并简要说明语境或语气区别。

3. **短语、句子或段落**
   * 英文或其他外文 → 译为自然、准确、符合现代汉语习惯的简体中文；
   * 中文 → 译为自然、地道、用词准确的英文；
   * 中英混杂文本 → 结合整体语境判断目标语言，保持专业术语、品牌名、代码标识符、路径与占位符原样，不强行逐词硬译。

4. **通用要求**
   * 忠实传达原文语气与逻辑，保留原有的段落、列表或代码格式；
   * 不解释判断或翻译过程，不输出前言、总结或免责声明，不使用 Markdown 表格。

## 输出格式

* **外文单词**：
  `/音标/`（如适用）
  **词性**：释义1；释义2
  **用法**：简短说明（仅必要时）

* **中文词语**：
  **英文**：translation 1（词性）
  **英文**：translation 2（词性）
  **区别**：简短说明（仅必要时）

* **短语、句子或段落**：
  直接输出译文正文，不要添加“翻译：”、“译文：”或额外引号。

## 待处理文本

{{text}}"#,
        ),
        action(
            "summary",
            "总结",
            "align-left",
            true,
            "请用简洁清晰的中文提炼下面的核心内容，直接输出要点，不写开场白：\n\n1. 先用**一句话概括主旨**；\n2. 再分条列出关键事实、依据或结论（若原文较短，直接给出 1~2 句精炼总结即可）；\n3. 忠于原文，不臆测或补充未提及的信息。\n\n待总结内容：\n\n{{text}}",
        ),
        action(
            "explain",
            "解释",
            "message-circle-question",
            true,
            "请用清晰、通俗的中文解释下面的内容，直接进入正题，避免空泛套话：\n\n1. **核心含义**：先用直白的话说明它是什么、表达了什么核心意思；\n2. **关键拆解**：若是专业术语、代码、公式或报错信息，分点说明其关键部分、原理或常见原因；\n3. **简要例子**：仅在确实有助于理解时补充 1 个简短例子，简单概念无需举例。\n\n待解释内容：\n\n{{text}}",
        ),
        action(
            "refine",
            "润色",
            "sparkles",
            false,
            "在不改变原意的前提下润色下面的文本，使其更流畅、准确、自然，并直接输出润色后的最终文本：\n\n1. **保持原意与原语种**：不增删核心信息，不把原文翻译成其他语言；\n2. **贴合语气与格式**：保留原有的专业术语、表达风格以及段落、列表或代码等格式；\n3. **只输出正文**：不要添加“润色如下：”、引号、修改说明或任何额外备注。\n\n待润色文本：\n\n{{text}}",
        ),
    ]
}

fn action(id: &str, name: &str, icon: &str, enabled: bool, prompt: &str) -> ActionItem {
    ActionItem {
        id: id.into(),
        name: name.into(),
        icon: icon.into(),
        enabled,
        built_in: true,
        prompt: prompt.into(),
    }
}
