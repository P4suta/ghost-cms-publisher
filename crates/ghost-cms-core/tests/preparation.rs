//! Offline preparation contracts for complete publication collections.

use ghost_cms_core::CoreError;
use ghost_cms_core::preparation::PrepareOptions;
use ghost_cms_shared::filesystem::prepare_collection;

#[test]
fn article_routes_stay_on_the_configured_site() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let first = temp.path().join("first.md");
    let second = temp.path().join("second.md");
    std::fs::write(
        &first,
        "---\ntitle: First\nslug: first-post\n---\nSee [second](second.md) here.\n",
    )?;
    std::fs::write(
        &second,
        "---\ntitle: Second\nslug: second-post\n---\nText.\n",
    )?;
    let files = [first, second];
    for (template, expected) in [
        ("/{slug}/", "/second-post/"),
        ("/articles/{slug}/", "/articles/second-post/"),
        ("/旅行/{slug}/", "/%E6%97%85%E8%A1%8C/second-post/"),
    ] {
        let collection = prepare_collection(
            &files,
            PrepareOptions {
                route_template: template.to_owned(),
                ..PrepareOptions::default()
            },
        )?;
        assert!(
            collection.posts()[0]
                .rendered_html()
                .contains(&format!("href=\"{expected}\"")),
            "incorrect article route for {template:?}"
        );
    }
    for template in [
        "//outside.invalid/{slug}/",
        "///outside.invalid/{slug}/",
        "/\t/outside.invalid/{slug}/",
        "/\r/outside.invalid/{slug}/",
        "/\n/outside.invalid/{slug}/",
        "https://outside.invalid/{slug}/",
        "relative/{slug}/",
        "/{slug}/{slug}/",
        "/articles/",
        "/{slug}/?preview",
        "/{slug}/#heading",
        "/\\outside.invalid/{slug}/",
    ] {
        let error = prepare_collection(
            &files,
            PrepareOptions {
                route_template: template.to_owned(),
                ..PrepareOptions::default()
            },
        )
        .expect_err("route must remain an absolute path on the configured site");
        assert!(error.to_string().contains("route template"));
    }
    Ok(())
}

#[test]
fn starter_media_limit_uses_five_million_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let image = temp.path().join("image.jpg");
    let file = std::fs::File::create(&image)?;
    let post = temp.path().join("post.md");
    std::fs::write(
        &post,
        "---\ntitle: Size\nslug: size-boundary\n---\n![A view](image.jpg)\n",
    )?;
    file.set_len(5_000_000)?;
    prepare_collection(std::slice::from_ref(&post), PrepareOptions::default())?;
    file.set_len(5_000_001)?;
    let Err(CoreError::Preparation(error)) = prepare_collection(&[post], PrepareOptions::default())
    else {
        return Err("expected the Starter media ceiling".into());
    };
    assert!(
        error
            .report
            .errors
            .iter()
            .any(|e| e.message.contains("5000000"))
    );
    Ok(())
}

#[test]
fn standalone_descriptive_links_become_cards_without_fetching_their_hosts()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let post = temp.path().join("links.md");
    std::fs::write(temp.path().join("poster.jpg"), b"poster")?;
    std::fs::write(
        &post,
        include_str!("fixtures/preparation/standalone-links.md"),
    )?;
    let collection = prepare_collection(&[post], PrepareOptions::default())?;
    let html = collection.posts()[0].rendered_html();
    assert_eq!(html.matches("kg-bookmark-card").count(), 2);
    let expected = include_str!("fixtures/preparation/standalone-links.expected");
    assert_ne!(expected.trim(), "");
    for fragment in expected.lines() {
        assert!(
            html.contains(fragment),
            "missing bookmark fragment: {fragment}"
        );
    }
    assert!(!html.contains("<blockquote>"));
    assert!(
        html.contains("See <a href=\"https://example.invalid/inline\">another museum</a> here.")
    );
    assert!(html.contains("<p><a href=\"https://example.invalid/url-only\">"));
    assert!(html.contains(">Open map</a></p>"));
    assert_eq!(html.matches("<img ").count(), 1);
    assert!(html.contains("kg-bookmark-thumbnail"));
    assert_eq!(collection.report().posts[0].rendering.link_cards, 2);
    assert!(html.contains("[Literal](https://example.invalid/code)"));
    Ok(())
}

#[test]
fn media_labels_become_escaped_captions_and_video_posters_stay_paired()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    std::fs::write(temp.path().join("photo.jpg"), b"photo")?;
    std::fs::write(temp.path().join("poster.jpg"), b"poster")?;
    std::fs::write(temp.path().join("clip.mp4"), b"video")?;
    std::fs::write(temp.path().join("audio.aac"), b"audio")?;
    let post = temp.path().join("media.md");
    std::fs::write(
        &post,
        include_str!("fixtures/preparation/media-captions.md"),
    )?;
    let collection = prepare_collection(&[post], PrepareOptions::default())?;
    let html = collection.posts()[0].rendered_html();
    let expected = include_str!("fixtures/preparation/media-captions.expected");
    assert_ne!(expected.trim(), "");
    for fragment in expected.lines() {
        assert!(html.contains(fragment), "missing media caption: {fragment}");
    }
    assert!(html.contains("<video controls"));
    assert!(html.contains("<audio controls"));
    assert!(html.contains(" poster=\"https://ghost-cms.invalid/assets/"));
    assert_eq!(html.matches("<img ").count(), 1);
    assert_eq!(html.matches("<figcaption>").count(), 2);
    assert!(html.contains("kg-video-card") && html.contains("kg-audio-card"));
    assert!(html.contains("Before") && html.contains("after."));
    assert!(html.contains("![](literal.jpg)"));
    let report = collection.report();
    assert_eq!(report.posts[0].rendering.media_captions, 3);
    assert_eq!(report.posts[0].rendering.uncaptioned_media, 0);
    Ok(())
}

#[test]
fn links_inside_lists_remain_links_in_their_original_order()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("list.md");
    std::fs::write(
        &file,
        "---\ntitle: Routes\nslug: routes\n---\n\n- [First museum](https://example.com/first)\n- [Second museum](https://example.com/second)\n",
    )?;
    let collection = prepare_collection(&[file], PrepareOptions::default())?;
    let html = collection.posts()[0].rendered_html();
    assert!(!html.contains("kg-bookmark-card"));
    assert!(html.contains("<li><a href=\"https://example.com/first\">First museum</a></li>"));
    assert!(html.find("First museum") < html.find("Second museum"));
    Ok(())
}

#[test]
fn preview_resolves_encoded_media_and_multilingual_article_fragments()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let first = temp.path().join("first.md");
    let second = temp.path().join("second.md");
    std::fs::write(temp.path().join("日本 旅行.png"), b"image bytes")?;
    std::fs::write(
        &first,
        include_str!("fixtures/preparation/unicode-first.md"),
    )?;
    std::fs::write(
        &second,
        include_str!("fixtures/preparation/unicode-second.md"),
    )?;
    let collection = prepare_collection(&[first.clone(), second], PrepareOptions::default())?;
    assert_eq!(collection.report().posts.len(), 2);
    assert_eq!(collection.report().posts[0].local_assets, 1);
    let output = temp.path().join("preview");
    collection.write_preview(&output)?;
    let first_html = std::fs::read_to_string(output.join("first-post/index.html"))?;
    assert!(first_html.contains("lang=\"zh-Hans\""));
    assert!(first_html.contains("../second-post/index.html#%E8%B7%AF%E7%BA%BF-1"));
    assert!(first_html.contains("../assets/"));
    assert!(first_html.contains("![not a reference](missing.png)"));
    assert!(!first_html.contains("ghost-cms.invalid"));
    assert_eq!(std::fs::read_dir(output.join("assets"))?.count(), 1);
    assert!(collection.write_preview(&output).is_err());
    assert!(output.join("index.html").is_file());
    assert!(prepare_collection(&[first], PrepareOptions::default()).is_err());
    Ok(())
}

#[test]
fn validation_reports_all_invalid_files_and_duplicate_slugs()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let paths: Vec<_> = ["first", "duplicate", "missing", "invalid"]
        .iter()
        .map(|n| temp.path().join(format!("{n}.md")))
        .collect();
    std::fs::write(
        &paths[0],
        "---\ntitle: One\nslug: same-slug\n---\n\nText.\n",
    )?;
    std::fs::write(
        &paths[1],
        "---\ntitle: Two\nslug: same-slug\n---\n\nText.\n",
    )?;
    std::fs::write(
        &paths[2],
        "---\ntitle: Missing\nslug: missing-image\n---\n\n![Image](missing.png)\n",
    )?;
    std::fs::write(
        &paths[3],
        "---\ntitle: Invalid\nslug: ../../outside\n---\n\nText.\n",
    )?;
    let Err(CoreError::Preparation(error)) = prepare_collection(&paths, PrepareOptions::default())
    else {
        return Err("expected a structured preparation error".into());
    };
    assert_eq!(error.report.errors.len(), 3);
    let messages = error
        .report
        .errors
        .iter()
        .map(|e| e.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("duplicate slug"));
    assert!(messages.contains("missing.png"));
    assert!(messages.contains("slug must"));
    Ok(())
}

#[test]
fn raw_html_requires_explicit_permission_but_code_examples_stay_literal()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("post.md");
    std::fs::write(
        &file,
        "---\ntitle: HTML\nslug: html-example\n---\n\n<span>Raw markup</span>\n",
    )?;
    assert!(prepare_collection(std::slice::from_ref(&file), PrepareOptions::default()).is_err());
    prepare_collection(
        std::slice::from_ref(&file),
        PrepareOptions {
            allow_raw_html: true,
            ..PrepareOptions::default()
        },
    )?;
    std::fs::write(
        &file,
        "---\ntitle: HTML\nslug: html-example\n---\n\n<img src=\"missing.png\">\n",
    )?;
    assert!(
        prepare_collection(
            std::slice::from_ref(&file),
            PrepareOptions {
                allow_raw_html: true,
                ..PrepareOptions::default()
            }
        )
        .is_err()
    );
    std::fs::write(
        &file,
        "---\ntitle: HTML\nslug: html-example\n---\n\n<iframe src=\"https://example.com/embed\"></iframe>\n",
    )?;
    prepare_collection(
        std::slice::from_ref(&file),
        PrepareOptions {
            allow_raw_html: true,
            ..PrepareOptions::default()
        },
    )?;
    std::fs::write(
        &file,
        "---\ntitle: HTML\nslug: html-example\n---\n\n```html\n<span>Raw markup</span>\n```\n",
    )?;
    prepare_collection(&[file], PrepareOptions::default())?;
    Ok(())
}

#[test]
fn preview_failure_cleans_its_own_output_and_preserves_existing_directories()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let post = temp.path().join("post.md");
    let image = temp.path().join("image.png");
    std::fs::write(&image, b"original image")?;
    std::fs::write(
        &post,
        "---\ntitle: Preview\nslug: preview-post\n---\n\n![Image](image.png)\n",
    )?;
    let collection = prepare_collection(&[post], PrepareOptions::default())?;
    std::fs::write(&image, b"changed image!")?;
    let output = temp.path().join("preview");
    assert!(collection.write_preview(&output).is_err());
    assert!(!output.exists());
    std::fs::create_dir(&output)?;
    std::fs::write(output.join("keep.txt"), b"keep")?;
    assert!(collection.write_preview(&output).is_err());
    assert_eq!(std::fs::read(output.join("keep.txt"))?, b"keep");
    Ok(())
}
