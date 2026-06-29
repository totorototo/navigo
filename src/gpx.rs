use crate::waypoint::Waypoint;
use crate::Location;

// ── Byte-scanning helpers ─────────────────────────────────────────────────────

fn find_from(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| from + offset)
}

fn find_byte_from(haystack: &[u8], from: usize, byte: u8) -> Option<usize> {
    haystack
        .get(from..)?
        .iter()
        .position(|&b| b == byte)
        .map(|offset| from + offset)
}

/// Extracts the value of an attribute like `lat="..."` or `lat='...'` from
/// `tag_section` (the bytes of the opening tag only).  `base` is the offset
/// of `tag_section[0]` inside the full `bytes` slice, which is needed to
/// locate the closing quote in the correct position.
fn parse_attr(bytes: &[u8], base: usize, tag_section: &[u8], attr: &[u8]) -> Option<f64> {
    let attr_offset = tag_section.windows(attr.len()).position(|w| w == attr)?;
    let opening_quote_index = base + attr_offset + attr.len();
    let opening_quote = *bytes.get(opening_quote_index)?;
    let value_start = opening_quote_index + 1;
    let value_end = find_byte_from(bytes, value_start, opening_quote)?;
    let value_str = std::str::from_utf8(bytes.get(value_start..value_end)?).ok()?;
    value_str.parse::<f64>().ok()
}

/// Returns a subslice of `bytes` for the text between `open_tag` and
/// `close_tag`, searching only within `content` but returning indices into
/// the full `bytes` buffer.  Returns `None` when the closing tag falls outside
/// the parent element that ends at `element_end`.
fn parse_tag_content<'a>(
    bytes: &'a [u8],
    content: &[u8],
    content_start: usize,
    element_end: usize,
    open_tag: &[u8],
    close_tag: &[u8],
) -> Option<&'a [u8]> {
    let relative_pos = content
        .windows(open_tag.len())
        .position(|w| w == open_tag)?;
    let value_start = content_start + relative_pos + open_tag.len();
    let value_end = find_from(bytes, value_start, close_tag)?;
    if value_end > element_end {
        return None;
    }
    bytes.get(value_start..value_end)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse `<trkpt>` elements from raw GPX bytes into a flat `Vec<Location>`.
///
/// Handles both single-quoted and double-quoted attribute values.  Malformed
/// or incomplete track-points are silently skipped, matching the behaviour of
/// the reference Zig implementation.
pub fn parse_trace_points(bytes: &[u8]) -> Vec<Location> {
    let mut locations = Vec::with_capacity(bytes.len() / 100);
    let mut pos = 0;

    while let Some(trkpt_start) = find_from(bytes, pos, b"<trkpt") {
        let trkpt_end = match find_from(bytes, trkpt_start, b"</trkpt>") {
            Some(index) => index,
            None => break,
        };
        pos = trkpt_end + b"</trkpt>".len();

        let tag_end = match find_from(bytes, trkpt_start, b">") {
            Some(index) => index,
            None => continue,
        };

        let tag_section = match bytes.get(trkpt_start..tag_end) {
            Some(s) => s,
            None => continue,
        };
        let content_start = tag_end + 1;
        let content = match bytes.get(content_start..trkpt_end) {
            Some(s) => s,
            None => continue,
        };

        let lat = match parse_attr(bytes, trkpt_start, tag_section, b"lat=") {
            Some(value) => value,
            None => continue,
        };
        let lon = match parse_attr(bytes, trkpt_start, tag_section, b"lon=") {
            Some(value) => value,
            None => continue,
        };
        let ele_bytes = match parse_tag_content(
            bytes,
            content,
            content_start,
            trkpt_end,
            b"<ele>",
            b"</ele>",
        ) {
            Some(slice) => slice,
            None => continue,
        };
        let elevation = match std::str::from_utf8(ele_bytes)
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
        {
            Some(value) => value,
            None => continue,
        };

        locations.push(Location {
            longitude: lon,
            latitude: lat,
            altitude: elevation,
        });
    }

    locations
}

/// Metadata parsed from the `<metadata>` section (or root-level fallback).
pub struct GpxMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// Combined GPX payload used by the full WASM analysis path.
#[cfg(any(feature = "wasm", test))]
pub(crate) struct ParsedGpx {
    pub locations: Vec<Location>,
    pub waypoints: Vec<Waypoint>,
    pub metadata: GpxMetadata,
}

/// Parse track points, waypoints and metadata in a single pass.
#[cfg(any(feature = "wasm", test))]
pub(crate) fn parse_all(bytes: &[u8]) -> ParsedGpx {
    let mut locations = Vec::with_capacity(bytes.len() / 100);
    let mut waypoints = Vec::new();
    let mut metadata = GpxMetadata {
        name: None,
        description: None,
    };
    // Pre-scan: if a <metadata> block exists anywhere, root-level <name>/<desc>
    // must be ignored — matching the semantics of `parse_metadata`.
    let has_metadata_block = find_from(bytes, 0, b"<metadata>").is_some();

    let mut pos = 0usize;

    while let Some(tag_start) = find_byte_from(bytes, pos, b'<') {
        if bytes
            .get(tag_start..)
            .is_some_and(|s| s.starts_with(b"<trkpt"))
        {
            let trkpt_end = match find_from(bytes, tag_start, b"</trkpt>") {
                Some(index) => index,
                None => break,
            };
            pos = trkpt_end + b"</trkpt>".len();

            let tag_end = match find_from(bytes, tag_start, b">") {
                Some(index) => index,
                None => continue,
            };

            let tag_section = match bytes.get(tag_start..tag_end) {
                Some(s) => s,
                None => continue,
            };
            let content_start = tag_end + 1;
            let content = match bytes.get(content_start..trkpt_end) {
                Some(s) => s,
                None => continue,
            };

            let lat = match parse_attr(bytes, tag_start, tag_section, b"lat=") {
                Some(value) => value,
                None => continue,
            };
            let lon = match parse_attr(bytes, tag_start, tag_section, b"lon=") {
                Some(value) => value,
                None => continue,
            };
            let ele_bytes = match parse_tag_content(
                bytes,
                content,
                content_start,
                trkpt_end,
                b"<ele>",
                b"</ele>",
            ) {
                Some(slice) => slice,
                None => continue,
            };
            let elevation = match std::str::from_utf8(ele_bytes)
                .ok()
                .and_then(|s| s.parse::<f64>().ok())
            {
                Some(value) => value,
                None => continue,
            };

            locations.push(Location {
                longitude: lon,
                latitude: lat,
                altitude: elevation,
            });
            continue;
        }

        if bytes
            .get(tag_start..)
            .is_some_and(|s| s.starts_with(b"<wpt"))
        {
            let wpt_end = match find_from(bytes, tag_start, b"</wpt>") {
                Some(index) => index,
                None => break,
            };
            pos = wpt_end + b"</wpt>".len();

            let tag_end = match find_from(bytes, tag_start, b">") {
                Some(index) if index < wpt_end => index,
                _ => continue,
            };
            let tag_section = match bytes.get(tag_start..tag_end) {
                Some(s) => s,
                None => continue,
            };
            let content_start = tag_end + 1;
            let content = match bytes.get(content_start..wpt_end) {
                Some(s) => s,
                None => continue,
            };

            let lat = match parse_attr(bytes, tag_start, tag_section, b"lat=") {
                Some(value) => value,
                None => continue,
            };
            let lon = match parse_attr(bytes, tag_start, tag_section, b"lon=") {
                Some(value) => value,
                None => continue,
            };

            let elevation =
                parse_tag_content(bytes, content, content_start, wpt_end, b"<ele>", b"</ele>")
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .and_then(|s| s.parse::<f64>().ok());

            let name = parse_tag_content(
                bytes,
                content,
                content_start,
                wpt_end,
                b"<name>",
                b"</name>",
            )
            .and_then(|s| std::str::from_utf8(s).ok())
            .map(String::from)
            .unwrap_or_default();

            let description = parse_tag_content(
                bytes,
                content,
                content_start,
                wpt_end,
                b"<desc>",
                b"</desc>",
            )
            .and_then(|s| std::str::from_utf8(s).ok())
            .map(String::from);

            let comment =
                parse_tag_content(bytes, content, content_start, wpt_end, b"<cmt>", b"</cmt>")
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .map(String::from);

            let symbol =
                parse_tag_content(bytes, content, content_start, wpt_end, b"<sym>", b"</sym>")
                    .and_then(|s| std::str::from_utf8(s).ok())
                    .map(String::from);

            let wpt_type = parse_tag_content(
                bytes,
                content,
                content_start,
                wpt_end,
                b"<type>",
                b"</type>",
            )
            .and_then(|s| std::str::from_utf8(s).ok())
            .map(String::from);

            let time = parse_tag_content(
                bytes,
                content,
                content_start,
                wpt_end,
                b"<time>",
                b"</time>",
            )
            .and_then(|s| std::str::from_utf8(s).ok())
            .and_then(|s| crate::time::parse_iso8601_to_epoch(s).ok());

            let stop_duration = parse_tag_content(
                bytes,
                content,
                content_start,
                wpt_end,
                b"<stopDuration>",
                b"</stopDuration>",
            )
            .and_then(|s| std::str::from_utf8(s).ok())
            .and_then(|s| s.parse::<u32>().ok());

            waypoints.push(Waypoint {
                latitude: lat,
                longitude: lon,
                elevation,
                name,
                description,
                comment,
                symbol,
                wpt_type,
                time,
                stop_duration,
            });
            continue;
        }

        if bytes
            .get(tag_start..)
            .is_some_and(|s| s.starts_with(b"<metadata>"))
        {
            let metadata_end = match find_from(bytes, tag_start, b"</metadata>") {
                Some(index) => index,
                None => break,
            };
            let metadata_content = &bytes[tag_start..metadata_end];

            if let Some(name_pos) = metadata_content.windows(6).position(|w| w == b"<name>") {
                let value_start = tag_start + name_pos + 6;
                if let Some(value_end) = find_from(bytes, value_start, b"</name>") {
                    if value_end <= metadata_end {
                        metadata.name = std::str::from_utf8(&bytes[value_start..value_end])
                            .ok()
                            .map(String::from);
                    }
                }
            }

            if let Some(desc_pos) = metadata_content.windows(6).position(|w| w == b"<desc>") {
                let value_start = tag_start + desc_pos + 6;
                if let Some(value_end) = find_from(bytes, value_start, b"</desc>") {
                    if value_end <= metadata_end {
                        metadata.description = std::str::from_utf8(&bytes[value_start..value_end])
                            .ok()
                            .map(String::from);
                    }
                }
            }

            pos = metadata_end + b"</metadata>".len();
            continue;
        }

        if !has_metadata_block
            && metadata.name.is_none()
            && bytes
                .get(tag_start..)
                .is_some_and(|s| s.starts_with(b"<name>"))
        {
            let value_start = tag_start + 6;
            if let Some(value_end) = find_from(bytes, value_start, b"</name>") {
                metadata.name = std::str::from_utf8(&bytes[value_start..value_end])
                    .ok()
                    .map(String::from);
            }
            pos = value_start;
            continue;
        }

        if !has_metadata_block
            && metadata.description.is_none()
            && bytes
                .get(tag_start..)
                .is_some_and(|s| s.starts_with(b"<desc>"))
        {
            let value_start = tag_start + 6;
            if let Some(value_end) = find_from(bytes, value_start, b"</desc>") {
                metadata.description = std::str::from_utf8(&bytes[value_start..value_end])
                    .ok()
                    .map(String::from);
            }
            pos = value_start;
            continue;
        }

        pos = tag_start + 1;
    }

    ParsedGpx {
        locations,
        waypoints,
        metadata,
    }
}

/// Parse `<metadata>` name and description from raw GPX bytes.
///
/// Falls back to root-level `<name>` / `<desc>` when no `<metadata>` block is present.
pub fn parse_metadata(bytes: &[u8]) -> GpxMetadata {
    let mut metadata = GpxMetadata {
        name: None,
        description: None,
    };

    if let Some(metadata_start) = find_from(bytes, 0, b"<metadata>") {
        if let Some(metadata_end) = find_from(bytes, metadata_start, b"</metadata>") {
            let metadata_content = &bytes[metadata_start..metadata_end];

            if let Some(name_pos) = metadata_content.windows(6).position(|w| w == b"<name>") {
                let value_start = metadata_start + name_pos + 6;
                if let Some(value_end) = find_from(bytes, value_start, b"</name>") {
                    if value_end <= metadata_end {
                        metadata.name = std::str::from_utf8(&bytes[value_start..value_end])
                            .ok()
                            .map(String::from);
                    }
                }
            }

            if let Some(desc_pos) = metadata_content.windows(6).position(|w| w == b"<desc>") {
                let value_start = metadata_start + desc_pos + 6;
                if let Some(value_end) = find_from(bytes, value_start, b"</desc>") {
                    if value_end <= metadata_end {
                        metadata.description = std::str::from_utf8(&bytes[value_start..value_end])
                            .ok()
                            .map(String::from);
                    }
                }
            }
        }
    } else {
        if let Some(name_start) = find_from(bytes, 0, b"<name>") {
            let value_start = name_start + 6;
            if let Some(value_end) = find_from(bytes, value_start, b"</name>") {
                metadata.name = std::str::from_utf8(&bytes[value_start..value_end])
                    .ok()
                    .map(String::from);
            }
        }
        if let Some(desc_start) = find_from(bytes, 0, b"<desc>") {
            let value_start = desc_start + 6;
            if let Some(value_end) = find_from(bytes, value_start, b"</desc>") {
                metadata.description = std::str::from_utf8(&bytes[value_start..value_end])
                    .ok()
                    .map(String::from);
            }
        }
    }

    metadata
}

/// Parse `<wpt>` elements from raw GPX bytes into a `Vec<Waypoint>`.
///
/// Waypoints without `lat` or `lon` are skipped. All other fields are optional.
pub fn parse_waypoints(bytes: &[u8]) -> Vec<Waypoint> {
    let mut waypoints = Vec::new();
    let mut pos = 0;

    while let Some(wpt_start) = find_from(bytes, pos, b"<wpt") {
        let wpt_end = match find_from(bytes, wpt_start, b"</wpt>") {
            Some(index) => index,
            None => break,
        };
        pos = wpt_end + b"</wpt>".len();

        let tag_end = match find_from(bytes, wpt_start, b">") {
            Some(index) if index < wpt_end => index,
            _ => continue,
        };
        let tag_section = match bytes.get(wpt_start..tag_end) {
            Some(s) => s,
            None => continue,
        };
        let content_start = tag_end + 1;
        let content = match bytes.get(content_start..wpt_end) {
            Some(s) => s,
            None => continue,
        };

        let lat = match parse_attr(bytes, wpt_start, tag_section, b"lat=") {
            Some(value) => value,
            None => continue,
        };
        let lon = match parse_attr(bytes, wpt_start, tag_section, b"lon=") {
            Some(value) => value,
            None => continue,
        };

        let elevation =
            parse_tag_content(bytes, content, content_start, wpt_end, b"<ele>", b"</ele>")
                .and_then(|s| std::str::from_utf8(s).ok())
                .and_then(|s| s.parse::<f64>().ok());

        let name = parse_tag_content(
            bytes,
            content,
            content_start,
            wpt_end,
            b"<name>",
            b"</name>",
        )
        .and_then(|s| std::str::from_utf8(s).ok())
        .map(String::from)
        .unwrap_or_default();

        let description = parse_tag_content(
            bytes,
            content,
            content_start,
            wpt_end,
            b"<desc>",
            b"</desc>",
        )
        .and_then(|s| std::str::from_utf8(s).ok())
        .map(String::from);

        let comment =
            parse_tag_content(bytes, content, content_start, wpt_end, b"<cmt>", b"</cmt>")
                .and_then(|s| std::str::from_utf8(s).ok())
                .map(String::from);

        let symbol = parse_tag_content(bytes, content, content_start, wpt_end, b"<sym>", b"</sym>")
            .and_then(|s| std::str::from_utf8(s).ok())
            .map(String::from);

        let wpt_type = parse_tag_content(
            bytes,
            content,
            content_start,
            wpt_end,
            b"<type>",
            b"</type>",
        )
        .and_then(|s| std::str::from_utf8(s).ok())
        .map(String::from);

        let time = parse_tag_content(
            bytes,
            content,
            content_start,
            wpt_end,
            b"<time>",
            b"</time>",
        )
        .and_then(|s| std::str::from_utf8(s).ok())
        .and_then(|s| crate::time::parse_iso8601_to_epoch(s).ok());

        let stop_duration = parse_tag_content(
            bytes,
            content,
            content_start,
            wpt_end,
            b"<stopDuration>",
            b"</stopDuration>",
        )
        .and_then(|s| std::str::from_utf8(s).ok())
        .and_then(|s| s.parse::<u32>().ok());

        waypoints.push(Waypoint {
            latitude: lat,
            longitude: lon,
            elevation,
            name,
            description,
            comment,
            symbol,
            wpt_type,
            time,
            stop_duration,
        });
    }

    waypoints
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_track_points() {
        let gpx = br#"<?xml version="1.0"?>
<gpx>
  <trk><trkseg>
    <trkpt lat="37.123456" lon="-122.123456"><ele>100.0</ele></trkpt>
    <trkpt lat="37.123556" lon="-122.123556"><ele>150.0</ele></trkpt>
    <trkpt lat="37.123656" lon="-122.123656"><ele>200.0</ele></trkpt>
  </trkseg></trk>
</gpx>"#;

        let locations = parse_trace_points(gpx);
        assert_eq!(locations.len(), 3);
        assert_eq!(locations[0].latitude, 37.123456);
        assert_eq!(locations[0].longitude, -122.123456);
        assert_eq!(locations[0].altitude, 100.0);
        assert_eq!(locations[2].altitude, 200.0);
    }

    #[test]
    fn handles_single_quoted_attributes() {
        let gpx = b"<trkpt lat='48.5' lon='2.3'><ele>300</ele></trkpt>";
        let locations = parse_trace_points(gpx);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].latitude, 48.5);
        assert_eq!(locations[0].longitude, 2.3);
        assert_eq!(locations[0].altitude, 300.0);
    }

    #[test]
    fn skips_malformed_points() {
        let gpx = br#"<gpx>
  <trk><trkseg>
    <trkpt lon="-122.0"><ele>100</ele></trkpt>
    <trkpt lat="37.0" lon="-122.0"><ele>200</ele></trkpt>
    <trkpt lat="not_a_number" lon="-122.0"><ele>300</ele></trkpt>
  </trkseg></trk>
</gpx>"#;

        let locations = parse_trace_points(gpx);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].altitude, 200.0);
    }

    #[test]
    fn preserves_high_precision_coordinates() {
        let gpx =
            b"<trkpt lat=\"37.123456789\" lon=\"-122.987654321\"><ele>123.456789</ele></trkpt>";
        let locations = parse_trace_points(gpx);
        assert_eq!(locations.len(), 1);
        assert!((locations[0].latitude - 37.123456789).abs() < 1e-9);
        assert!((locations[0].longitude - -122.987654321).abs() < 1e-9);
        assert!((locations[0].altitude - 123.456789).abs() < 1e-6);
    }

    #[test]
    fn returns_empty_for_no_track_points() {
        let gpx = b"<gpx><metadata><name>No track</name></metadata></gpx>";
        let locations = parse_trace_points(gpx);
        assert!(locations.is_empty());
    }

    #[test]
    fn parse_metadata_extracts_name_and_description() {
        let gpx = br#"<?xml version="1.0"?>
<gpx>
  <metadata>
    <name>My Trail</name>
    <desc>A beautiful route</desc>
  </metadata>
</gpx>"#;
        let m = parse_metadata(gpx);
        assert_eq!(m.name.as_deref(), Some("My Trail"));
        assert_eq!(m.description.as_deref(), Some("A beautiful route"));
    }

    #[test]
    fn parse_metadata_handles_missing_block() {
        let gpx = b"<gpx><trk><trkseg></trkseg></trk></gpx>";
        let m = parse_metadata(gpx);
        assert!(m.name.is_none());
        assert!(m.description.is_none());
    }

    #[test]
    fn parse_metadata_falls_back_to_root_name() {
        let gpx = b"<gpx><name>Root Name</name><trk></trk></gpx>";
        let m = parse_metadata(gpx);
        assert_eq!(m.name.as_deref(), Some("Root Name"));
    }

    #[test]
    fn parse_waypoints_basic() {
        let gpx = br#"<?xml version="1.0"?>
<gpx>
  <wpt lat="45.0" lon="7.0">
    <name>Checkpoint 1</name>
    <type>TimeBarrier</type>
  </wpt>
  <wpt lat="45.5" lon="7.5">
    <name>Life Base</name>
    <type>LifeBase</type>
    <stopDuration>1800</stopDuration>
  </wpt>
</gpx>"#;
        let waypoints = parse_waypoints(gpx);
        assert_eq!(waypoints.len(), 2);
        assert_eq!(waypoints[0].latitude, 45.0);
        assert_eq!(waypoints[0].longitude, 7.0);
        assert_eq!(waypoints[0].name, "Checkpoint 1");
        assert_eq!(waypoints[0].wpt_type.as_deref(), Some("TimeBarrier"));
        assert_eq!(waypoints[1].stop_duration, Some(1800));
        assert_eq!(waypoints[1].wpt_type.as_deref(), Some("LifeBase"));
    }

    #[test]
    fn parse_waypoints_skips_missing_lat_or_lon() {
        let gpx = br#"<gpx>
  <wpt lon="7.0"><name>No lat</name></wpt>
  <wpt lat="45.0" lon="7.0"><name>Valid</name></wpt>
</gpx>"#;
        let waypoints = parse_waypoints(gpx);
        assert_eq!(waypoints.len(), 1);
        assert_eq!(waypoints[0].name, "Valid");
    }

    #[test]
    fn parse_waypoints_parses_iso8601_time() {
        let gpx = b"<wpt lat=\"45.0\" lon=\"7.0\"><name>Start</name><type>Start</type><time>2025-11-20T12:00:00Z</time></wpt>";
        let waypoints = parse_waypoints(gpx);
        assert_eq!(waypoints.len(), 1);
        assert_eq!(waypoints[0].time, Some(1763640000));
    }

    #[test]
    fn parse_waypoints_missing_name_yields_empty_string() {
        let gpx = b"<wpt lat=\"45.0\" lon=\"7.0\"></wpt>";
        let waypoints = parse_waypoints(gpx);
        assert_eq!(waypoints.len(), 1);
        assert_eq!(waypoints[0].name, "");
    }

    #[test]
    fn parse_waypoints_desc_not_shared_between_waypoints() {
        let gpx = br#"<gpx>
  <wpt lat="45.0" lon="7.0"><name>First</name></wpt>
  <wpt lat="45.1" lon="7.1"><name>Second</name><desc>Only second has this</desc></wpt>
</gpx>"#;
        let waypoints = parse_waypoints(gpx);
        assert_eq!(waypoints.len(), 2);
        assert!(waypoints[0].description.is_none());
        assert_eq!(
            waypoints[1].description.as_deref(),
            Some("Only second has this")
        );
    }

    #[test]
    fn parse_all_does_not_fall_back_to_root_name_when_metadata_block_exists() {
        let gpx = br#"<?xml version="1.0"?>
<gpx>
  <name>Root Name</name>
  <metadata>
    <desc>Track description</desc>
  </metadata>
  <trk><trkseg>
    <trkpt lat="45.0" lon="7.0"><ele>100</ele></trkpt>
  </trkseg></trk>
</gpx>"#;

        let parsed = parse_all(gpx);
        assert!(parsed.metadata.name.is_none());
        assert_eq!(
            parsed.metadata.description.as_deref(),
            Some("Track description")
        );
        assert_eq!(parsed.locations.len(), 1);
        assert!(parsed.waypoints.is_empty());
    }
}
