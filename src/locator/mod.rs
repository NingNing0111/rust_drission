//! Locator 解析：支持 css / xpath / text / attr / id / class / tag 字符串格式

use std::fmt;

/// 定位器类型（README §7）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Locator {
    Css(String),
    XPath(String),
    Text(String),
    Attr(String, String),
    Id(String),
    Class(String),
    Tag(String),
}

impl Locator {
    /// 从字符串解析。
    /// 格式：`css:.btn` / `xpath://div` / `text:立即沟通` / `attr:data-id=123` / `id:login` / `class:card` / `tag:div`。
    /// 若无前缀且以 `#`、`.` 开头或首字符为字母/`*`/`[`，则按 CSS 选择器解析（如 `#kw`、`.btn`、`input[name=q]`）。
    pub fn parse(s: &str) -> Result<Self, LocatorParseError> {
        let s = s.trim();
        if let Some(colon) = s.find(':') {
            let (kind, rest) = s.split_at(colon);
            let rest = rest[1..].trim();
            if rest.is_empty() {
                return Err(LocatorParseError::EmptyValue);
            }
            match kind.to_lowercase().as_str() {
                "css" => return Ok(Locator::Css(rest.to_string())),
                "xpath" => return Ok(Locator::XPath(rest.to_string())),
                "text" => return Ok(Locator::Text(rest.to_string())),
                "attr" => {
                    let eq = rest.find('=').ok_or(LocatorParseError::AttrNoEquals)?;
                    let (name, value) = rest.split_at(eq);
                    return Ok(Locator::Attr(
                        name.trim().to_string(),
                        value[1..].trim().to_string(),
                    ));
                }
                "id" => return Ok(Locator::Id(rest.to_string())),
                "class" => return Ok(Locator::Class(rest.to_string())),
                "tag" => return Ok(Locator::Tag(rest.to_string())),
                _ => return Err(LocatorParseError::UnknownKind(kind.to_string())),
            }
        }
        if s.is_empty() {
            return Err(LocatorParseError::EmptyValue);
        }
        let first = s.chars().next().unwrap();
        if first == '#' || first == '.' || first.is_ascii_alphabetic() || first == '*' || first == '['
        {
            Ok(Locator::Css(s.to_string()))
        } else {
            Err(LocatorParseError::MissingPrefix)
        }
    }

    /// 转为 CSS 选择器（用于 DOM.querySelector）。XPath / Text 在调用方用 evaluate 或 performSearch 处理
    pub fn to_css_selector(&self) -> Option<String> {
        match self {
            Locator::Css(s) => Some(s.clone()),
            Locator::Id(s) => Some(format!("#{}", escape_id_selector(s))),
            Locator::Class(s) => Some(format!(".{}", escape_class_selector(s))),
            Locator::Tag(s) => Some(s.clone()),
            Locator::Attr(name, value) => Some(format!("[{}=\"{}\"]", name, escape_attr_value(value))),
            Locator::XPath(_) | Locator::Text(_) => None,
        }
    }

    /// 是否为 XPath（需用 Runtime.evaluate 或 performSearch 等处理）
    pub fn is_xpath(&self) -> bool {
        matches!(self, Locator::XPath(_))
    }

    /// 是否为纯文本定位（需用 XPath 或 evaluate 处理）
    pub fn is_text(&self) -> bool {
        matches!(self, Locator::Text(_))
    }

    /// 得到用于 evaluate 的 XPath 表达式（仅当 is_xpath 或 is_text 时使用）
    pub fn to_xpath_expression(&self) -> Option<String> {
        match self {
            Locator::XPath(s) => Some(s.clone()),
            Locator::Text(s) => {
                let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
                Some(format!("//*[contains(text(),'{}')]", escaped))
            }
            _ => None,
        }
    }

    /// 得到用于 DOM.performSearch 的查询字符串（CSS 或 XPath），可搜索整棵 DOM 树（含 iframe）
    pub fn to_search_query(&self) -> Option<String> {
        self.to_css_selector().or_else(|| self.to_xpath_expression())
    }
}

fn escape_id_selector(s: &str) -> String {
    // CSS 中 id 选择器需转义特殊字符
    s.replace('\\', "\\\\").replace('#', "\\#")
}

fn escape_class_selector(s: &str) -> String {
    s.replace('\\', "\\\\").replace('.', "\\.")
}

fn escape_attr_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug, thiserror::Error)]
pub enum LocatorParseError {
    #[error("Missing locator prefix. Expected one of: css:, xpath:, text:, attr:, id:, class:, tag:")]
    MissingPrefix,
    #[error("Locator value is empty after the prefix")]
    EmptyValue,
    #[error("Invalid attr locator format. Expected name=value")]
    AttrNoEquals,
    #[error("Unknown locator type: {0}")]
    UnknownKind(String),
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Locator::Css(s) => write!(f, "css:{}", s),
            Locator::XPath(s) => write!(f, "xpath:{}", s),
            Locator::Text(s) => write!(f, "text:{}", s),
            Locator::Attr(a, b) => write!(f, "attr:{}={}", a, b),
            Locator::Id(s) => write!(f, "id:{}", s),
            Locator::Class(s) => write!(f, "class:{}", s),
            Locator::Tag(s) => write!(f, "tag:{}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locator_parse_css() {
        let loc = Locator::parse("css:.btn").unwrap();
        assert!(matches!(loc, Locator::Css(ref s) if s == ".btn"));
        assert_eq!(loc.to_css_selector().as_deref(), Some(".btn"));
        assert!(!loc.is_xpath());
        assert!(!loc.is_text());
    }

    #[test]
    fn locator_parse_css_with_spaces() {
        let loc = Locator::parse("  css: div.main  ").unwrap();
        assert!(matches!(loc, Locator::Css(ref s) if s == "div.main"));
    }

    #[test]
    fn locator_parse_xpath() {
        let loc = Locator::parse("xpath://div[@id='foo']").unwrap();
        assert!(matches!(loc, Locator::XPath(ref s) if s == "//div[@id='foo']"));
        assert!(loc.is_xpath());
        assert_eq!(loc.to_xpath_expression().as_deref(), Some("//div[@id='foo']"));
        assert!(loc.to_css_selector().is_none());
    }

    #[test]
    fn locator_parse_text() {
        let loc = Locator::parse("text:立即沟通").unwrap();
        assert!(matches!(loc, Locator::Text(ref s) if s == "立即沟通"));
        assert!(loc.is_text());
        let xpath = loc.to_xpath_expression().unwrap();
        assert!(xpath.contains("contains(text(),"));
        assert!(xpath.contains("立即沟通"));
    }

    #[test]
    fn locator_parse_attr() {
        let loc = Locator::parse("attr:data-id=123").unwrap();
        assert!(matches!(loc, Locator::Attr(ref a, ref b) if a == "data-id" && b == "123"));
        assert_eq!(loc.to_css_selector().as_deref(), Some("[data-id=\"123\"]"));
    }

    #[test]
    fn locator_parse_id() {
        let loc = Locator::parse("id:login").unwrap();
        assert!(matches!(loc, Locator::Id(ref s) if s == "login"));
        assert_eq!(loc.to_css_selector().as_deref(), Some("#login"));
    }

    #[test]
    fn locator_parse_class() {
        let loc = Locator::parse("class:card").unwrap();
        assert!(matches!(loc, Locator::Class(ref s) if s == "card"));
        assert_eq!(loc.to_css_selector().as_deref(), Some(".card"));
    }

    #[test]
    fn locator_parse_tag() {
        let loc = Locator::parse("tag:div").unwrap();
        assert!(matches!(loc, Locator::Tag(ref s) if s == "div"));
        assert_eq!(loc.to_css_selector().as_deref(), Some("div"));
    }

    #[test]
    fn locator_parse_case_insensitive() {
        let loc = Locator::parse("CSS:.btn").unwrap();
        assert!(matches!(loc, Locator::Css(_)));
        let loc = Locator::parse("XPATH://div").unwrap();
        assert!(matches!(loc, Locator::XPath(_)));
    }

    #[test]
    fn locator_parse_bare_css() {
        assert!(matches!(Locator::parse("#kw"), Ok(Locator::Css(s)) if s == "#kw"));
        assert!(matches!(Locator::parse(".btn"), Ok(Locator::Css(s)) if s == ".btn"));
        assert!(matches!(Locator::parse("input[name=q]"), Ok(Locator::Css(s)) if s == "input[name=q]"));
    }

    #[test]
    fn locator_parse_error_missing_prefix() {
        assert!(matches!(
            Locator::parse("123"),
            Err(LocatorParseError::MissingPrefix)
        ));
    }

    #[test]
    fn locator_parse_error_empty_value() {
        assert!(matches!(
            Locator::parse("css:"),
            Err(LocatorParseError::EmptyValue)
        ));
        assert!(matches!(
            Locator::parse("css:   "),
            Err(LocatorParseError::EmptyValue)
        ));
    }

    #[test]
    fn locator_parse_error_attr_no_equals() {
        assert!(matches!(
            Locator::parse("attr:novalue"),
            Err(LocatorParseError::AttrNoEquals)
        ));
    }

    #[test]
    fn locator_parse_error_unknown_kind() {
        assert!(matches!(
            Locator::parse("unknown:value"),
            Err(LocatorParseError::UnknownKind(s)) if s == "unknown"
        ));
    }

    #[test]
    fn locator_display() {
        assert_eq!(Locator::parse("css:.btn").unwrap().to_string(), "css:.btn");
        assert_eq!(Locator::parse("id:foo").unwrap().to_string(), "id:foo");
    }

    #[test]
    fn locator_id_selector_escape() {
        let loc = Locator::parse("id:foo#bar").unwrap();
        let sel = loc.to_css_selector().unwrap();
        assert!(sel.contains("\\#"));
    }
}
