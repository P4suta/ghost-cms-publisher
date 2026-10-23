use std::collections::BTreeMap;

use comrak::nodes::{AstNode, NodeHtmlBlock, NodeValue};
use comrak::{Options, format_html};
use scraper::{Html, Selector};
use serde::Serialize;

use crate::{CoreError, Result};

const GENERATED_HTML_BLOCK_TYPE: u8 = 6;

/// Visible media and bookmark inventory in prepared HTML.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RenderReport {
    /// Media figures with a nonempty visible caption.
    pub media_captions: usize,
    /// Media figures without a visible caption, including decorative images.
    pub uncaptioned_media: usize,
    /// Standalone links rendered as bookmark cards.
    pub link_cards: usize,
}

pub(super) fn inventory(html: &str) -> Result<RenderReport> {
    let select = |selector| {
        Selector::parse(selector).map_err(|error| {
            CoreError::FrontMatter(format!("cannot inspect rendered media: {error}"))
        })
    };
    let document = Html::parse_fragment(html);
    let caption = select("figcaption")?;
    let mut report = RenderReport::default();
    for figure in document.select(&select(
        "figure.kg-image-card, figure.kg-native-video, figure.kg-native-audio",
    )?) {
        if figure
            .select(&caption)
            .any(|e| !e.text().collect::<String>().trim().is_empty())
        {
            report.media_captions += 1;
        } else {
            report.uncaptioned_media += 1;
        }
    }
    report.link_cards = document.select(&select("figure.kg-bookmark-card")?).count();
    Ok(report)
}

#[derive(Clone, Copy)]
pub(super) enum MediaKind {
    Video,
    Audio,
}

pub(super) fn classify(url: &str) -> Option<MediaKind> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    match crate::media::content_type(std::path::Path::new(path)) {
        mime if mime.starts_with("video/") => Some(MediaKind::Video),
        mime if mime.starts_with("audio/") => Some(MediaKind::Audio),
        _ => None,
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn render_node<'a>(node: &'a AstNode<'a>, options: &Options<'_>) -> Result<String> {
    let mut html = String::new();
    format_html(node, options, &mut html)
        .map_err(|e| CoreError::FrontMatter(format!("cannot render media: {e}")))?;
    Ok(html)
}

fn image_node<'a>(node: &'a AstNode<'a>) -> Option<&'a AstNode<'a>> {
    if matches!(node.data().value, NodeValue::Image(_)) {
        return Some(node);
    }
    if matches!(node.data().value, NodeValue::Link(_)) {
        let mut children = node.children();
        let child = children.next()?;
        if children.next().is_none() && matches!(child.data().value, NodeValue::Image(_)) {
            return Some(child);
        }
    }
    None
}

fn media_link(
    node: &AstNode<'_>,
    types: &BTreeMap<String, MediaKind>,
) -> Option<(String, MediaKind)> {
    let data = node.data();
    let NodeValue::Link(link) = &data.value else {
        return None;
    };
    types.get(&link.url).map(|kind| (link.url.clone(), *kind))
}

fn media_figure<'a>(
    node: &'a AstNode<'a>,
    url: &str,
    kind: MediaKind,
    poster: Option<&str>,
    options: &Options<'_>,
) -> Result<String> {
    let label = escape(&comrak::html::collect_text(node));
    let fallback = render_node(node, options)?;
    let tag = match kind {
        MediaKind::Video => "video",
        MediaKind::Audio => "audio",
    };
    let poster = poster
        .map(|url| format!(" poster=\"{}\"", escape(url)))
        .unwrap_or_default();
    Ok(format!(
        "<figure class=\"kg-card kg-embed-card kg-native-{tag}\"><{tag} controls preload=\"metadata\" src=\"{}\"{poster} aria-label=\"{label}\">{fallback}</{tag}><figcaption>{label}</figcaption></figure>\n",
        escape(url)
    ))
}

fn flush(paragraph: &mut String, html: &mut String) {
    if !paragraph.trim().is_empty() {
        html.push_str("<p>");
        html.push_str(paragraph);
        html.push_str("</p>\n");
    }
    paragraph.clear();
}

fn set_html<'a>(node: &'a AstNode<'a>, html: String) {
    let children = node.children().collect::<Vec<_>>();
    for child in children {
        child.detach();
    }
    node.data_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock {
        block_type: GENERATED_HTML_BLOCK_TYPE,
        literal: html,
    });
}

fn single_child<'a>(node: &'a AstNode<'a>) -> Option<&'a AstNode<'a>> {
    let mut children = node.children();
    let child = children.next()?;
    children.next().is_none().then_some(child)
}

fn bookmark_link<'a>(node: &'a AstNode<'a>) -> Option<(String, String, String)> {
    let data = node.data();
    let NodeValue::Link(link) = &data.value else {
        return None;
    };
    let title = comrak::html::collect_text(node).trim().to_owned();
    if title.is_empty()
        || title == link.url
        || title.starts_with("http://")
        || title.starts_with("https://")
        || matches!(
            title.to_ascii_lowercase().as_str(),
            "open map" | "website" | "link" | "here"
        )
        || image_node(node).is_some()
        || classify(&link.url).is_some()
        || link.url.starts_with(super::ASSET_PREFIX)
    {
        return None;
    }
    let publisher = if let Ok(url) = reqwest::Url::parse(&link.url) {
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        url.host_str()?.to_owned()
    } else if link.url.starts_with('/') || link.url.starts_with("../") {
        String::new()
    } else {
        return None;
    };
    Some((link.url.clone(), title, publisher))
}

fn bookmark<'a>(
    paragraph: &'a AstNode<'a>,
    node: &'a AstNode<'a>,
    poster: Option<&str>,
) -> Option<String> {
    let (url, title, publisher) = bookmark_link(node)?;
    let mut href = String::new();
    comrak::html::escape_href(&mut href, &url, false).ok()?;
    let mut description = match &node.data().value {
        NodeValue::Link(link) => link.title.clone(),
        _ => String::new(),
    };
    if let Some(next) = paragraph.next_sibling()
        && matches!(next.data().value, NodeValue::BlockQuote)
        && let Some(child) = single_child(next)
        && matches!(child.data().value, NodeValue::Paragraph)
        && child
            .descendants()
            .all(|n| !matches!(n.data().value, NodeValue::Image(_) | NodeValue::Link(_)))
    {
        description = comrak::html::collect_text(child);
        next.detach();
    }
    let description = if description.is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"kg-bookmark-description\">{}</div>",
            escape(&description)
        )
    };
    let poster = poster.map(|src| format!("<div class=\"kg-bookmark-thumbnail\"><img src=\"{}\" alt=\"\" loading=\"lazy\"></div>", escape(src))).unwrap_or_default();
    Some(format!(
        "<figure class=\"kg-card kg-bookmark-card\"><a class=\"kg-bookmark-container\" href=\"{href}\"><div class=\"kg-bookmark-content\"><div class=\"kg-bookmark-title\">{}</div>{description}<div class=\"kg-bookmark-metadata\"><span class=\"kg-bookmark-publisher\">{}</span></div></div>{poster}</a></figure>\n",
        escape(&title),
        escape(&publisher)
    ))
}

fn bookmark_poster<'a>(paragraph: &'a AstNode<'a>) -> bool {
    let Some(image) = single_child(paragraph).and_then(image_node) else {
        return false;
    };
    let Some(next) = paragraph.next_sibling() else {
        return false;
    };
    if !matches!(next.data().value, NodeValue::Paragraph) {
        return false;
    }
    let Some(link) = single_child(next) else {
        return false;
    };
    let Some((url, label, _)) = bookmark_link(link) else {
        return false;
    };
    let external_video = reqwest::Url::parse(&url).is_ok_and(|url| {
        matches!(
            url.host_str(),
            Some("www.youtube.com" | "youtube.com" | "youtu.be" | "vimeo.com" | "www.vimeo.com")
        )
    });
    if !external_video || label != comrak::html::collect_text(image) {
        return false;
    }
    let poster = match &image.data().value {
        NodeValue::Image(link) => link.url.clone(),
        _ => return false,
    };
    let Some(html) = bookmark(next, link, Some(&poster)) else {
        return false;
    };
    next.detach();
    set_html(paragraph, html);
    true
}

fn poster_pair<'a>(
    paragraph: &'a AstNode<'a>,
    types: &BTreeMap<String, MediaKind>,
    options: &Options<'_>,
) -> Result<bool> {
    let children = paragraph.children().collect::<Vec<_>>();
    if children.len() != 1 {
        return Ok(false);
    }
    let Some(image) = image_node(children[0]) else {
        return Ok(false);
    };
    let Some(next) = paragraph.next_sibling() else {
        return Ok(false);
    };
    if !matches!(next.data().value, NodeValue::Paragraph) {
        return Ok(false);
    }
    let label = comrak::html::collect_text(image);
    if label.is_empty() {
        return Ok(false);
    }
    let mut links = Vec::new();
    for node in next.children() {
        if let Some((url, MediaKind::Video)) = media_link(node, types) {
            if !comrak::html::collect_text(node).starts_with(&label) {
                return Ok(false);
            }
            links.push((node, url));
        } else if !matches!(&node.data().value, NodeValue::Text(text) if text.trim().is_empty())
            && !matches!(node.data().value, NodeValue::SoftBreak)
        {
            return Ok(false);
        }
    }
    if links.is_empty() {
        return Ok(false);
    }
    let poster = match &image.data().value {
        NodeValue::Image(link) => link.url.clone(),
        _ => return Ok(false),
    };
    let mut html = String::new();
    if links.len() > 1 {
        html.push_str("<div class=\"kg-media-clips\">\n");
    }
    for (index, (node, url)) in links.iter().enumerate() {
        html.push_str(&media_figure(
            node,
            url,
            MediaKind::Video,
            (index == 0).then_some(poster.as_str()),
            options,
        )?);
    }
    if links.len() > 1 {
        html.push_str("</div>\n");
    }
    next.detach();
    set_html(paragraph, html);
    Ok(true)
}

pub(super) fn render<'a>(
    root: &'a AstNode<'a>,
    options: &Options<'_>,
    types: &BTreeMap<String, MediaKind>,
) -> Result<()> {
    let paragraphs = root
        .descendants()
        .filter(|node| matches!(node.data().value, NodeValue::Paragraph))
        .collect::<Vec<_>>();
    for paragraph in paragraphs {
        if paragraph.parent().is_none() || !matches!(paragraph.data().value, NodeValue::Paragraph) {
            continue;
        }
        if poster_pair(paragraph, types, options)? {
            continue;
        }
        if bookmark_poster(paragraph) {
            continue;
        }
        if let Some(link) = single_child(paragraph)
            && let Some(html) = bookmark(paragraph, link, None)
        {
            set_html(paragraph, html);
            continue;
        }
        let children = paragraph.children().collect::<Vec<_>>();
        if !children
            .iter()
            .any(|node| image_node(node).is_some() || media_link(node, types).is_some())
        {
            continue;
        }
        let mut html = String::new();
        let mut inline = String::new();
        for node in children {
            if let Some(image) = image_node(node) {
                flush(&mut inline, &mut html);
                let image_html = render_node(node, options)?;
                let caption = escape(&comrak::html::collect_text(image));
                let caption = if caption.is_empty() {
                    String::new()
                } else {
                    format!("<figcaption>{caption}</figcaption>")
                };
                html.push_str("<figure class=\"kg-card kg-image-card\">");
                html.push_str(&image_html);
                html.push_str(&caption);
                html.push_str("</figure>\n");
            } else if let Some((url, kind)) = media_link(node, types) {
                flush(&mut inline, &mut html);
                html.push_str(&media_figure(node, &url, kind, None, options)?);
            } else {
                inline.push_str(&render_node(node, options)?);
            }
        }
        flush(&mut inline, &mut html);
        set_html(paragraph, html);
    }
    Ok(())
}
