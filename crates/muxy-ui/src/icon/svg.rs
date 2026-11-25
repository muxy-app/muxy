use std::collections::HashMap;

const MAX_EXPANDED_NODES: u32 = 10_000;

pub const PAGE_ICON_SIDE: u32 = 64;

pub fn within_budget(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        nodes_limit: MAX_EXPANDED_NODES,
    };
    let Ok(document) = roxmltree::Document::parse_with_options(text, options) else {
        return false;
    };
    let mut ids: HashMap<&str, roxmltree::NodeId> = HashMap::new();
    for node in document.descendants() {
        if let Some(id) = node.attribute("id")
            && ids.insert(id, node.id()).is_some()
        {
            return false;
        }
        if !allowed(node) {
            return false;
        }
    }
    let mut sizes = HashMap::new();
    expanded(document.root_element(), &document, &ids, &mut sizes)
        .is_some_and(|size| size <= MAX_EXPANDED_NODES)
}

fn allowed(node: roxmltree::Node<'_, '_>) -> bool {
    match node.tag_name().name() {
        "pattern" | "filter" => false,
        "style" => !node
            .descendants()
            .filter_map(|text| text.text())
            .any(|text| text.contains("url(")),
        _ => true,
    }
}

fn expanded<'a>(
    node: roxmltree::Node<'a, 'a>,
    document: &'a roxmltree::Document<'a>,
    ids: &HashMap<&str, roxmltree::NodeId>,
    sizes: &mut HashMap<roxmltree::NodeId, Option<u32>>,
) -> Option<u32> {
    match sizes.get(&node.id()) {
        Some(Some(size)) => return Some(*size),
        Some(None) => return None,
        None => {}
    }
    if node.tag_name().name() == "marker" {
        return None;
    }
    sizes.insert(node.id(), None);
    let mut size: u32 = 1;
    for id in references(node) {
        if let Some(target) = ids.get(id) {
            let target = document.get_node(*target)?;
            size = size.checked_add(expanded(target, document, ids, sizes)?)?;
            if size > MAX_EXPANDED_NODES {
                return None;
            }
        }
    }
    if node.tag_name().name() != "use" {
        for child in node.children().filter(roxmltree::Node::is_element) {
            size = size.checked_add(expanded(child, document, ids, sizes)?)?;
            if size > MAX_EXPANDED_NODES {
                return None;
            }
        }
    }
    (size <= MAX_EXPANDED_NODES).then(|| {
        sizes.insert(node.id(), Some(size));
        size
    })
}

fn references<'a>(node: roxmltree::Node<'a, 'a>) -> Vec<&'a str> {
    let mut ids = Vec::new();
    for attribute in node.attributes() {
        let value = attribute.value();
        if attribute.name() == "href" {
            ids.extend(value.trim().strip_prefix('#'));
            continue;
        }
        let mut rest = value;
        while let Some(start) = rest.find("url(") {
            rest = &rest[start + 4..];
            let end = rest.find(')').unwrap_or(rest.len());
            let target = rest[..end].trim().trim_matches(['"', '\'']).trim();
            ids.extend(target.strip_prefix('#'));
            rest = &rest[end..];
        }
    }
    ids
}

pub fn fitted_document(bytes: &[u8], side: u32) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut document = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}"><image width="{side}" height="{side}" href="data:image/svg+xml,"#
    );
    for byte in bytes {
        let _ = write!(document, "%{byte:02X}");
    }
    document.push_str(r#""/></svg>"#);
    document.into_bytes()
}

pub fn page_icon(bytes: &[u8]) -> Option<gpui::Image> {
    within_budget(bytes).then(|| {
        gpui::Image::from_bytes(
            gpui::ImageFormat::Svg,
            fitted_document(bytes, PAGE_ICON_SIDE),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 16 16">
        <defs><circle id="dot" cx="2" cy="2" r="1"/></defs>
        <use href="#dot"/><use xlink:href="#dot" x="4"/>
        <path d="M0 0h16v16H0z"/>
    </svg>"##;

    fn laughs() -> String {
        use std::fmt::Write as _;
        let mut svg = String::from(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><defs><path id="l0" d="M0 0h1"/>"#,
        );
        for level in 1..=10 {
            let _ = write!(svg, r#"<g id="l{level}">"#);
            for _ in 0..10 {
                let _ = write!(svg, r##"<use href="#l{}"/>"##, level - 1);
            }
            svg.push_str("</g>");
        }
        svg.push_str(r##"</defs><use href="#l10"/></svg>"##);
        svg
    }

    #[test]
    fn ordinary_icons_are_accepted() {
        assert!(within_budget(ICON.as_bytes()));
    }

    #[test]
    fn exponential_mask_clip_and_pattern_references_are_refused() {
        use std::fmt::Write as _;
        for (element, attribute) in [
            ("mask", "mask=\"url(#r{})\""),
            ("clipPath", "clip-path=\"url(#r{})\""),
            ("pattern", "style=\"fill: url('#r{}')\""),
        ] {
            let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg"><defs>"#);
            let _ = write!(
                svg,
                r#"<{element} id="r8"><rect width="1" height="1"/></{element}>"#
            );
            for level in (0..8).rev() {
                let _ = write!(svg, r#"<{element} id="r{level}">"#);
                for _ in 0..10 {
                    let reference = attribute.replace("{}", &(level + 1).to_string());
                    let _ = write!(svg, r#"<rect width="1" height="1" {reference}/>"#);
                }
                let _ = write!(svg, "</{element}>");
            }
            let first = attribute.replace("{}", "0");
            let _ = write!(
                svg,
                r#"</defs><rect width="16" height="16" {first}/></svg>"#
            );
            assert!(svg.len() < 6 * 1024, "{element}: {}", svg.len());
            assert!(!within_budget(svg.as_bytes()), "{element}");
        }
        let shared = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
            <defs><linearGradient id="g"><stop offset="0"/><stop offset="1"/></linearGradient>
            <clipPath id="c"><rect width="16" height="16"/></clipPath></defs>
            <rect width="4" height="4" fill="url(#g)" clip-path="url(#c)"/>
            <rect x="8" width="4" height="4" style="fill:url('#g')"/>
        </svg>"#;
        assert!(within_budget(shared.as_bytes()));
    }

    #[test]
    fn duplicate_ids_patterns_filters_and_stylesheet_references_are_refused() {
        let duplicate = laughs().replace("</svg>", r#"<g id="l10"/><g id="l9"/></svg>"#);
        assert!(!within_budget(duplicate.as_bytes()));
        let repeated = r#"<svg xmlns="http://www.w3.org/2000/svg"><g id="a"/><g id="a"/></svg>"#;
        assert!(!within_budget(repeated.as_bytes()));
        for refused in [
            r#"<svg xmlns="http://www.w3.org/2000/svg"><defs><pattern id="p" width="8192" height="8192" patternUnits="userSpaceOnUse"><rect width="1" height="1"/></pattern></defs><rect width="16" height="16" fill="url(#p)"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><defs><filter id="f"><feGaussianBlur stdDeviation="9"/></filter></defs><rect width="16" height="16" filter="url(#f)"/></svg>"#,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.a { mask: url(#m) }</style><mask id="m"><rect width="1" height="1"/></mask><rect class="a" width="16" height="16"/></svg>"#,
        ] {
            assert!(!within_budget(refused.as_bytes()), "{refused}");
        }
        let styled = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.a { fill: currentColor }</style><rect class="a" width="16" height="16"/></svg>"#;
        assert!(within_budget(styled.as_bytes()));
    }

    #[test]
    fn exponential_use_references_are_refused() {
        let svg = laughs();
        assert!(svg.len() < 4096);
        assert!(!within_budget(svg.as_bytes()));
    }

    #[test]
    fn use_cycles_are_refused() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
            <g id="a"><use href="#b"/></g><g id="b"><use href="#a"/></g></svg>"##;
        assert!(!within_budget(svg.as_bytes()));
    }

    #[test]
    fn markers_and_unreadable_files_are_refused() {
        let marker = r#"<svg xmlns="http://www.w3.org/2000/svg"><marker id="m"/></svg>"#;
        assert!(!within_budget(marker.as_bytes()));
        assert!(!within_budget(b"\x1f\x8b\x08\x00"));
        assert!(!within_budget(b"<svg"));
    }

    #[test]
    fn entity_expansion_is_counted() {
        use std::fmt::Write as _;
        let mut svg = String::from(
            r#"<!DOCTYPE svg [<!ENTITY e0 "<g/><g/><g/><g/><g/><g/><g/><g/><g/><g/>">"#,
        );
        for level in 1..6 {
            let previous = format!("&e{};", level - 1).repeat(10);
            let _ = write!(svg, r#"<!ENTITY e{level} "{previous}">"#);
        }
        svg.push_str(r#"]><svg xmlns="http://www.w3.org/2000/svg">&e5;</svg>"#);
        assert!(!within_budget(svg.as_bytes()));
    }

    #[gpui::test]
    fn a_huge_claimed_size_draws_into_a_small_bitmap(cx: &mut gpui::TestAppContext) {
        let huge = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="100000"><rect width="100000" height="100000"/></svg>"#;
        let image = page_icon(huge).expect("within budget");
        let rendered = cx
            .update(|cx| image.to_image_data(cx.svg_renderer()))
            .expect("renders");
        let size = rendered.size(0);
        let expected = i32::try_from(PAGE_ICON_SIDE).expect("small");
        assert_eq!((size.width.0, size.height.0), (expected, expected));
        let pixels = rendered.as_bytes(0).expect("one frame");
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
}
