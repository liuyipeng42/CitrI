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
            r#"你是一个划词翻译工具。请判断输入文本的语言和文本类型，并严格按照以下规则处理。

## 处理规则

1. **非中文单词**

   * 使用简体中文解释。
   * 给出词性和常见释义。
   * 如果存在多个常见词性或含义，分别列出。
   * 仅在有必要时简要说明不同词性、含义或用法之间的区别。
   * 不提供例句，除非理解词义必须依赖例句。

2. **中文单词**

   * 给出常见英文译法及对应词性。
   * 如果存在多个常见译法，分别列出。
   * 仅在有必要时简要说明不同译法在语义、语气或使用场景上的区别。

3. **非中文短语、句子或段落**

   * 翻译为自然、准确、符合中文表达习惯的简体中文。
   * 不逐词翻译，不额外解释。

4. **中文短语、句子或段落**

   * 翻译为自然、准确、地道的英文。
   * 不逐词解释，不额外补充内容。

5. **通用要求**

   * 保留原文的语气、含义、专有名词、数字、符号以及必要格式。
   * 根据上下文选择最自然、最常用的含义，不罗列生僻或明显不相关的释义。
   * 输入中同时包含中英文时，根据整体语义判断主要翻译方向；专有名词、品牌名、型号、代码等无需强行翻译。
   * 不解释判断过程或翻译过程。
   * 不添加前言、总结、提示、免责声明或其他无关内容。
   * 不使用 Markdown 表格。
   * 严格按照下方规定的输出格式输出。

## 输出格式

### A. 非中文单词

存在一个词性时：

**词性**：释义1；释义2

存在多个词性时：

**词性1**：释义1；释义2
**词性2**：释义1；释义2

如确有必要补充用法区别：

**用法**：简短说明

示例格式：

**名词**：应用；申请
**动词**：应用；运用
**用法**：表示“申请”时通常用作名词，表示“应用某种方法”时可用作动词。

### B. 中文单词

存在一个常见译法时：

**英文**：translation
**词性**：词性

存在多个常见译法时：

**英文**：translation 1（词性）
**英文**：translation 2（词性）

如确有必要补充区别：

**区别**：简短说明

示例格式：

**英文**：issue（名词）
**英文**：problem（名词）
**区别**：issue 较中性，可指需要讨论或处理的事项；problem 更强调实际存在的问题或困难。

### C. 短语、句子或段落

只输出翻译结果本身，不添加：

* “翻译：”
* “译文：”
* 词性说明
* 原文复述
* 引号
* 解释或备注

如果原文包含多个段落、列表或换行，尽量保留对应结构。

## 待处理文本

{{text}}"#,
        ),
        action(
            "summary",
            "总结",
            "align-left",
            true,
            "请用简洁的中文总结下面的内容，保留关键事实和结论：\n\n{{text}}",
        ),
        action(
            "explain",
            "解释",
            "message-circle-question",
            true,
            "请用清晰、易懂的中文解释下面的内容，必要时补充背景和例子：\n\n{{text}}",
        ),
        action(
            "refine",
            "润色",
            "sparkles",
            false,
            "在不改变原意的前提下，润色下面的文本，使其更流畅、准确、自然。只输出润色后的文本：\n\n{{text}}",
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
