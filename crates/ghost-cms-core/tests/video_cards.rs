//! Native video editor state and repair contracts.

use ghost_cms_core::domain::Post;
use serde_json::json;

use ghost_cms_core::video::{VideoCard, VideoPatch, patch_editor, same_content};
use std::collections::BTreeMap;

fn patch() -> BTreeMap<String, VideoPatch> {
    BTreeMap::from([(
        "https://example.invalid/clip.mp4".to_owned(),
        VideoPatch {
            src: "https://example.invalid/clip.mp4".to_owned(),
            thumbnail: "https://example.invalid/frame.jpg".to_owned(),
            width: 360,
            height: 640,
            duration_ms: 10_540,
            thumbnail_width: 360,
            thumbnail_height: 640,
            file_name: "clip.mp4".to_owned(),
            mime_type: "video/mp4".to_owned(),
        },
    )])
}

fn document() -> String {
    json!({"root":{"type":"root","version":1,"children":[
        {"type":"paragraph","children":[{"type":"extended-text","text":"A recorded visit.","format":0}]},
        {"type":"video","version":1,"src":"https://example.invalid/clip.mp4","caption":"A waterfall.","customThumbnailSrc":"","thumbnailSrc":"","width":null,"height":null,"duration":0,"loop":true},
        {"type":"image","src":"https://example.invalid/photo.jpg","caption":"An unchanged caption."}
    ]}}).to_string()
}

#[test]
fn repairs_populate_portrait_cards_without_changing_authored_content()
-> Result<(), Box<dyn std::error::Error>> {
    let before = document();
    let repaired = patch_editor(&before, &patch())?.to_string();
    assert!(same_content(&before, &repaired)?);
    let cards = ghost_cms_core::video::editor_videos(&repaired)?;
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].width, Some(360));
    assert_eq!(cards[0].height, Some(640));
    assert_eq!(cards[0].duration, 10.54);
    assert_eq!(cards[0].thumbnail_src, "https://example.invalid/frame.jpg");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&patch_editor(&repaired, &patch())?.to_string())?,
        serde_json::from_str::<serde_json::Value>(&repaired)?
    );
    let mut changed: serde_json::Value = serde_json::from_str(&repaired)?;
    changed["root"]["children"][1]["caption"] = "A different claim.".into();
    assert!(!same_content(&before, &changed.to_string())?);
    changed["root"]["children"][1]["src"] = "https://example.invalid/different.mp4".into();
    assert!(patch_editor(&changed.to_string(), &patch()).is_err());
    assert!(patch_editor(&before, &BTreeMap::new()).is_err());
    Ok(())
}

#[test]
fn authored_posters_survive_and_invalid_card_fields_fail() -> Result<(), Box<dyn std::error::Error>>
{
    let mut before: serde_json::Value = serde_json::from_str(&document())?;
    before["root"]["children"][1]["customThumbnailSrc"] =
        "https://example.invalid/authored.jpg".into();
    let repaired = patch_editor(&before.to_string(), &patch())?.to_string();
    let card = ghost_cms_core::video::editor_videos(&repaired)?.remove(0);
    assert_eq!(
        card.custom_thumbnail_src,
        "https://example.invalid/authored.jpg"
    );
    for (field, value) in [
        ("width", json!(0)),
        ("height", json!(null)),
        ("duration", json!(0)),
        ("thumbnailSrc", json!("file:///private/frame.jpg")),
    ] {
        let mut value_card = serde_json::to_value(&card)?;
        value_card["customThumbnailSrc"] = "".into();
        value_card[field] = value;
        assert!(
            serde_json::from_value::<VideoCard>(value_card)?
                .validate()
                .is_err(),
            "{field}"
        );
    }
    let mut unused = patch();
    unused.insert(
        "https://example.invalid/unused.mp4".to_owned(),
        VideoPatch {
            src: "https://example.invalid/unused.mp4".to_owned(),
            ..patch().into_values().next().ok_or("missing fixture")?
        },
    );
    assert!(patch_editor(&document(), &unused).is_err());
    assert!(patch_editor("{", &patch()).is_err());
    Ok(())
}

#[test]
fn a_video_url_without_a_thumbnail_is_not_a_populated_editor_card()
-> Result<(), Box<dyn std::error::Error>> {
    let post: Post = serde_json::from_value(json!({
        "id":"video-post", "slug":"video-post", "status":"draft",
        "lexical":json!({"root":{"type":"root","children":[
            {"type":"paragraph","children":[{"type":"text","text":"A recorded visit."}]},
            {"type":"video","version":1,"src":"https://example.invalid/clip.mp4",
             "thumbnailSrc":"","customThumbnailSrc":"","width":null,"height":null,"duration":0}
        ]}}).to_string()
    }))?;
    assert!(
        post.editor_blocks().is_err(),
        "a playable URL doesn't populate the Ghost editor's video selector"
    );
    Ok(())
}
