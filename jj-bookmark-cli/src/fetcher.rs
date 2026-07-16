//! 元数据抓取（roadmap Phase 4，唯一实现处）。GET 页面 → 解析 `<title>` /
//! `og:description` / `og:image`。带超时；失败降级（返回错误，由调用方决定是否阻塞）。

use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::time::Duration;

/// 抓取结果；任一字段可能缺失（页面未提供）。
#[derive(Debug, Default)]
pub struct Meta {
    pub title: Option<String>,
    pub excerpt: Option<String>,
    pub cover: Option<String>,
}

/// 抓取并解析页面元数据。`timeout` 覆盖连接+读取整体耗时。
pub fn fetch(url: &str, timeout: Duration) -> Result<Meta> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent("Mozilla/5.0 (compatible; jj-bookmark/1.0; +https://github.com/yigegongjiang/jj-bookmark)")
        .build()
        .context("failed to build HTTP client")?;

    let body = client
        .get(url)
        .send()
        .with_context(|| format!("request failed: {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP status error: {url}"))?
        .text()
        .context("failed to read response body")?;

    Ok(parse_meta(&body))
}

/// 从 HTML 提取元数据（纯函数，便于测试）。
fn parse_meta(html: &str) -> Meta {
    let doc = Html::parse_document(html);
    Meta {
        title: meta_content(&doc, "og:title", true)
            .or_else(|| title_tag(&doc))
            .map(clean),
        excerpt: meta_content(&doc, "og:description", true)
            .or_else(|| meta_content(&doc, "description", false))
            .map(clean),
        cover: meta_content(&doc, "og:image", true).map(clean),
    }
}

/// 取 `<meta property=.. content>`（og:*）或 `<meta name=.. content>` 的 content。
fn meta_content(doc: &Html, key: &str, by_property: bool) -> Option<String> {
    let attr = if by_property { "property" } else { "name" };
    let sel = Selector::parse(&format!("meta[{attr}=\"{key}\"]")).ok()?;
    doc.select(&sel)
        .find_map(|el| el.value().attr("content"))
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
}

fn title_tag(doc: &Html) -> Option<String> {
    let sel = Selector::parse("title").ok()?;
    doc.select(&sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .filter(|s| !s.trim().is_empty())
}

/// 折叠空白、去首尾空格。
fn clean(s: String) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prefers_og_then_falls_back() {
        let html = r#"<html><head>
            <title>Fallback Title</title>
            <meta property="og:title" content="OG Title">
            <meta property="og:description" content="OG Desc">
            <meta property="og:image" content="https://x/img.png">
        </head></html>"#;
        let m = parse_meta(html);
        assert_eq!(m.title.as_deref(), Some("OG Title"));
        assert_eq!(m.excerpt.as_deref(), Some("OG Desc"));
        assert_eq!(m.cover.as_deref(), Some("https://x/img.png"));
    }

    #[test]
    fn parse_falls_back_to_title_and_name_description() {
        let html = r#"<html><head>
            <title>  Only  Title  </title>
            <meta name="description" content="Name Desc">
        </head></html>"#;
        let m = parse_meta(html);
        assert_eq!(m.title.as_deref(), Some("Only Title")); // 空白折叠
        assert_eq!(m.excerpt.as_deref(), Some("Name Desc"));
        assert_eq!(m.cover, None);
    }
}
