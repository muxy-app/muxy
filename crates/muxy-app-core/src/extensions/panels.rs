//! What an extension can say about how its panel looks and floats, in its
//! manifest and at run time.

use serde::Deserialize;
use serde_json::{Map, Value};

use super::manifest::{PanelControl, PanelMode, PanelPosition};

/// The most panels, docked or detached, one extension keeps open at once.
pub const MAX_PANELS: usize = 32;
/// The most hit or drag regions a panel may set.
pub const MAX_REGIONS: usize = 64;
const MAX_INSTANCE: usize = 64;
const MAX_TITLE: usize = 512;
const MAX_EXTENT: f32 = 16384.0;

/// A rectangle in points, from the page's or the window's top left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Part of a detached panel's frame. Missing fields keep what the panel has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub struct FrameSpec {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

impl FrameSpec {
    /// `self` over `base`, field by field.
    #[must_use]
    pub fn over(self, base: [f32; 4]) -> [f32; 4] {
        [
            self.x.unwrap_or(base[0]),
            self.y.unwrap_or(base[1]),
            self.width.unwrap_or(base[2]),
            self.height.unwrap_or(base[3]),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct SizeSpec {
    pub width: f32,
    pub height: f32,
}

/// Whether Muxy draws a detached panel's card and header, or the page draws
/// everything itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelChrome {
    #[default]
    Standard,
    None,
}

/// Whether a detached panel stays open across projects, or belongs to the
/// project it was opened in and shows only there.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelScope {
    #[default]
    Global,
    Project,
}

/// A value a page sets, or clears back to its default.
#[derive(Clone, Debug, PartialEq)]
pub enum Change<T> {
    Set(T),
    Clear,
}

impl<T> Change<T> {
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Set(value) => Some(value),
            Self::Clear => None,
        }
    }
}

/// Changes to a panel. Each field left out keeps its current value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelOptions {
    pub instance: Option<String>,
    pub mode: Option<PanelMode>,
    pub position: Option<PanelPosition>,
    pub frame: Option<FrameSpec>,
    pub min_size: Option<SizeSpec>,
    pub max_size: Option<SizeSpec>,
    pub chrome: Option<PanelChrome>,
    pub transparent: Option<bool>,
    pub click_through: Option<bool>,
    pub movable: Option<bool>,
    pub resizable: Option<bool>,
    pub scope: Option<PanelScope>,
    pub title: Option<Change<String>>,
    pub hidden_controls: Option<Vec<PanelControl>>,
    /// Clearing them takes clicks everywhere again.
    pub hit_regions: Option<Change<Vec<Rect>>>,
    pub drag_regions: Option<Vec<Rect>>,
}

impl PanelOptions {
    /// Reads options sent by a page or script. Unknown fields are ignored so
    /// newer extensions keep working; known fields of the wrong shape fail.
    pub fn parse(value: &Value) -> Result<Self, String> {
        let fields = match value {
            Value::Null => return Ok(Self::default()),
            Value::Object(fields) => fields,
            _ => return Err("panel options must be an object".into()),
        };
        let min_size = size(fields, "minSize")?;
        let max_size = size(fields, "maxSize")?;
        if let (Some(min), Some(max)) = (min_size, max_size)
            && (min.width > max.width || min.height > max.height)
        {
            return Err("minSize is larger than maxSize".into());
        }
        Ok(Self {
            instance: instance(fields)?,
            mode: field(fields, "mode")?,
            position: field(fields, "position")?,
            frame: frame(fields)?,
            min_size,
            max_size,
            chrome: field(fields, "chrome")?,
            transparent: flag(fields, "transparent")?,
            click_through: flag(fields, "clickThrough")?,
            movable: flag(fields, "movable")?,
            resizable: flag(fields, "resizable")?,
            scope: field(fields, "scope")?,
            title: title(fields)?,
            hidden_controls: field(fields, "hiddenControls")?,
            hit_regions: match fields.get("hitRegions") {
                None => None,
                Some(Value::Null) => Some(Change::Clear),
                Some(value) => Some(Change::Set(regions("hitRegions", value)?)),
            },
            drag_regions: match fields.get("dragRegions") {
                None => None,
                Some(Value::Null) => Some(Vec::new()),
                Some(value) => Some(regions("dragRegions", value)?),
            },
        })
    }

    /// Whether these options change anything a docked panel shows.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A panel instance name: up to 64 letters, digits, `.`, `_`, or `-`.
pub fn valid_instance(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_INSTANCE
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn instance(fields: &Map<String, Value>) -> Result<Option<String>, String> {
    match fields.get("instance") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) if valid_instance(name) => Ok(Some(name.clone())),
        Some(_) => Err("instance must be 1 to 64 letters, digits, '.', '_', or '-'".into()),
    }
}

fn field<T: serde::de::DeserializeOwned>(
    fields: &Map<String, Value>,
    name: &str,
) -> Result<Option<T>, String> {
    match fields.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| format!("invalid {name}: {error}")),
    }
}

fn flag(fields: &Map<String, Value>, name: &str) -> Result<Option<bool>, String> {
    match fields.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("{name} must be true or false")),
    }
}

fn number(value: &Value, name: &str) -> Result<f32, String> {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Points beyond f32 range are rejected below"
    )]
    let number = value
        .as_f64()
        .map(|number| number as f32)
        .filter(|number| number.is_finite() && number.abs() <= MAX_EXTENT)
        .ok_or_else(|| format!("{name} must be a finite number"))?;
    Ok(number)
}

fn frame(fields: &Map<String, Value>) -> Result<Option<FrameSpec>, String> {
    let Some(value) = fields.get("frame").filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let Value::Object(frame) = value else {
        return Err("frame must be an object".into());
    };
    let part = |key: &str| {
        frame
            .get(key)
            .filter(|value| !value.is_null())
            .map(|value| number(value, &format!("frame.{key}")))
            .transpose()
    };
    let spec = FrameSpec {
        x: part("x")?,
        y: part("y")?,
        width: part("width")?,
        height: part("height")?,
    };
    if spec.width.is_some_and(|width| width <= 0.0)
        || spec.height.is_some_and(|height| height <= 0.0)
    {
        return Err("frame width and height must be positive".into());
    }
    Ok(Some(spec))
}

fn size(fields: &Map<String, Value>, name: &str) -> Result<Option<SizeSpec>, String> {
    let Some(value) = fields.get(name).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let width = number(&value["width"], &format!("{name}.width"))?;
    let height = number(&value["height"], &format!("{name}.height"))?;
    if width < 0.0 || height < 0.0 {
        return Err(format!("{name} must not be negative"));
    }
    Ok(Some(SizeSpec { width, height }))
}

fn title(fields: &Map<String, Value>) -> Result<Option<Change<String>>, String> {
    match fields.get("title") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(Change::Clear)),
        Some(Value::String(title)) => {
            let title: String = title.trim().chars().take(MAX_TITLE).collect();
            Ok(Some(if title.is_empty() {
                Change::Clear
            } else {
                Change::Set(title)
            }))
        }
        Some(_) => Err("title must be a string".into()),
    }
}

fn regions(name: &str, value: &Value) -> Result<Vec<Rect>, String> {
    let Value::Array(items) = value else {
        return Err(format!("{name} must be an array of rectangles"));
    };
    if items.len() > MAX_REGIONS {
        return Err(format!("{name} takes at most {MAX_REGIONS} rectangles"));
    }
    items
        .iter()
        .map(|item| {
            let rect = Rect {
                x: number(&item["x"], &format!("{name}[].x"))?,
                y: number(&item["y"], &format!("{name}[].y"))?,
                width: number(&item["width"], &format!("{name}[].width"))?,
                height: number(&item["height"], &format!("{name}[].height"))?,
            };
            if rect.width < 0.0 || rect.height < 0.0 {
                return Err(format!("{name} rectangles must not have a negative size"));
            }
            Ok(rect)
        })
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "Tests fail immediately and compare exact parsed values"
)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn options_read_every_field() {
        let options = PanelOptions::parse(&json!({
            "instance": "note-1",
            "mode": "detached",
            "position": "bottom",
            "frame": { "x": 10, "width": 200.5 },
            "minSize": { "width": 40, "height": 30 },
            "chrome": "none",
            "transparent": true,
            "clickThrough": true,
            "movable": false,
            "resizable": false,
            "scope": "project",
            "title": "  Note  ",
            "hiddenControls": ["close", "detach"],
            "hitRegions": [{ "x": 0, "y": 0, "width": 64, "height": 64 }],
            "dragRegions": null,
            "future": "ignored"
        }))
        .unwrap();
        assert_eq!(options.instance.as_deref(), Some("note-1"));
        assert_eq!(options.mode, Some(PanelMode::Detached));
        assert_eq!(options.position, Some(PanelPosition::Bottom));
        assert_eq!(
            options.frame,
            Some(FrameSpec {
                x: Some(10.0),
                width: Some(200.5),
                ..FrameSpec::default()
            })
        );
        assert_eq!(options.chrome, Some(PanelChrome::None));
        assert_eq!(options.scope, Some(PanelScope::Project));
        assert_eq!(options.title, Some(Change::Set("Note".into())));
        assert_eq!(
            options.hidden_controls,
            Some(vec![PanelControl::Close, PanelControl::Detach])
        );
        assert!(matches!(options.hit_regions, Some(Change::Set(regions)) if regions.len() == 1));
        assert_eq!(options.drag_regions, Some(Vec::new()));
        assert_eq!(options.movable, Some(false));
    }

    #[test]
    fn null_clears_and_absence_keeps() {
        let options = PanelOptions::parse(&json!({ "title": null, "hitRegions": null })).unwrap();
        assert_eq!(options.title, Some(Change::Clear));
        assert_eq!(options.hit_regions, Some(Change::Clear));
        assert!(PanelOptions::parse(&Value::Null).unwrap().is_empty());
        assert!(PanelOptions::parse(&json!({})).unwrap().is_empty());
    }

    #[test]
    fn bad_shapes_are_refused() {
        for bad in [
            json!("detached"),
            json!({ "instance": "a b" }),
            json!({ "instance": "x".repeat(65) }),
            json!({ "mode": "sideways" }),
            json!({ "frame": { "width": -1 } }),
            json!({ "frame": { "x": "left" } }),
            json!({ "frame": { "x": 1e9 } }),
            json!({ "transparent": "yes" }),
            json!({ "hitRegions": [{ "x": 0, "y": 0, "width": 1 }] }),
            json!({ "hitRegions": [{ "x": 0, "y": 0, "width": -1, "height": 1 }] }),
            json!({ "minSize": { "width": -2, "height": 1 } }),
            json!({ "minSize": { "width": 50, "height": 1 }, "maxSize": { "width": 10, "height": 9 } }),
        ] {
            assert!(PanelOptions::parse(&bad).is_err(), "{bad}");
        }
        let many: Vec<_> = (0..=MAX_REGIONS)
            .map(|_| json!({ "x": 0, "y": 0, "width": 1, "height": 1 }))
            .collect();
        assert!(PanelOptions::parse(&json!({ "dragRegions": many })).is_err());
    }

    #[test]
    fn a_partial_frame_keeps_the_rest() {
        let spec = FrameSpec {
            y: Some(5.0),
            height: Some(50.0),
            ..FrameSpec::default()
        };
        assert_eq!(spec.over([1.0, 2.0, 3.0, 4.0]), [1.0, 5.0, 3.0, 50.0]);
    }
}
