//! PDF image extraction — find and extract inline/XObject images.

use lopdf::{Dictionary, Document, Object, Stream};

use crate::models::bbox::BoundingBox;
use crate::models::chunks::ImageChunk;
use crate::EdgePdfError;

/// Extracted image data from a PDF page.
#[derive(Debug, Clone)]
pub struct ExtractedImage {
    /// Image chunk with bounding box info
    pub chunk: ImageChunk,
    /// Raw image data (decoded from stream)
    pub data: Vec<u8>,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Color space name
    pub color_space: String,
    /// Bits per component
    pub bits_per_component: u8,
    /// Filter name (e.g., "DCTDecode" for JPEG, "FlateDecode" for PNG)
    pub filter: String,
}

/// One discoverable Image XObject on a page (top-level or Pattern-nested).
struct ListedImage {
    /// 1-based index matching [`extract_image_data`].
    index: u32,
    width: u32,
    height: u32,
    stream: Stream,
}

/// Extract image chunks from a PDF page.
///
/// Scans the page's Resources/XObject dictionary for Image XObjects and
/// extracts their metadata (position, dimensions). Raw data is extracted lazily.
pub fn extract_image_chunks(
    doc: &Document,
    page_number: u32,
    page_id: lopdf::ObjectId,
) -> Result<Vec<ImageChunk>, EdgePdfError> {
    let page_dict = doc
        .get_object(page_id)
        .map_err(|e| EdgePdfError::PipelineError {
            stage: 1,
            message: format!("Failed to get page {}: {}", page_number, e),
        })?
        .as_dict()
        .map_err(|e| EdgePdfError::PipelineError {
            stage: 1,
            message: format!("Page {} is not a dictionary: {}", page_number, e),
        })?;

    let resources = match page_dict.get(b"Resources") {
        Ok(r) => resolve_obj(doc, r),
        Err(_) => return Ok(Vec::new()),
    };

    let resources_dict = match resources.as_dict() {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };

    let listed = list_page_images(doc, resources_dict);
    Ok(listed
        .into_iter()
        .map(|img| ImageChunk {
            bbox: BoundingBox::new(
                Some(page_number),
                0.0,
                0.0,
                img.width as f64,
                img.height as f64,
            ),
            index: Some(img.index),
            level: None,
            source: Default::default(),
        })
        .collect())
}

/// Get raw image data for a specific XObject (1-based index over page + Pattern images).
pub fn extract_image_data(
    doc: &Document,
    page_id: lopdf::ObjectId,
    image_index: u32,
) -> Result<Option<ExtractedImage>, EdgePdfError> {
    let page_dict = doc
        .get_object(page_id)
        .map_err(|e| EdgePdfError::PipelineError {
            stage: 1,
            message: format!("Failed to get page: {}", e),
        })?
        .as_dict()
        .map_err(|e| EdgePdfError::PipelineError {
            stage: 1,
            message: format!("Page is not a dictionary: {}", e),
        })?;

    let resources = match page_dict.get(b"Resources") {
        Ok(r) => resolve_obj(doc, r),
        Err(_) => return Ok(None),
    };

    let resources_dict = match resources.as_dict() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    let listed = list_page_images(doc, resources_dict);
    let Some(img) = listed.into_iter().find(|i| i.index == image_index) else {
        return Ok(None);
    };

    Ok(Some(extracted_from_stream(img.stream, image_index)))
}

/// Resolve the 1-based image index for the Image XObject painted by a tiling
/// Pattern.
///
/// First principles (ISO 32000 §8.7): a tiling pattern's `/BBox` defines the
/// pattern cell. Skia/Chromium often also embeds a 1×N or N×1 bleed strip —
/// those are not the painted cell content. Prefer the Image whose pixel size
/// matches `/BBox`, otherwise the sole non-strip Image in the pattern resources.
pub fn find_pattern_image_index(
    doc: &Document,
    resources: &Dictionary,
    pattern_name: &[u8],
) -> Option<u32> {
    let listed = list_page_images(doc, resources);
    let (pattern_images, bbox_w, bbox_h) =
        list_pattern_images_with_bbox(doc, resources, pattern_name)?;
    if pattern_images.is_empty() {
        return None;
    }

    // Prefer Image whose Width/Height match Pattern /BBox (pattern cell).
    let cell = pattern_images.iter().find(|img| {
        !is_bleed_strip(img.width, img.height)
            && bbox_w
                .map(|bw| (img.width as f64 - bw).abs() < 1.0)
                .unwrap_or(false)
            && bbox_h
                .map(|bh| (img.height as f64 - bh).abs() < 1.0)
                .unwrap_or(false)
    });
    let best = cell.or_else(|| {
        // Fall back to the only non-strip image in the pattern cell.
        let non_strips: Vec<_> = pattern_images
            .iter()
            .filter(|img| !is_bleed_strip(img.width, img.height))
            .collect();
        if non_strips.len() == 1 {
            Some(non_strips[0])
        } else {
            None
        }
    })?;

    listed
        .into_iter()
        .find(|img| {
            img.width == best.width
                && img.height == best.height
                && img.stream.content == best.stream.content
        })
        .map(|img| img.index)
}

/// 1-pixel-wide or 1-pixel-tall images are tiling bleed strips, not page content.
fn is_bleed_strip(width: u32, height: u32) -> bool {
    width <= 1 || height <= 1
}

fn list_pattern_images_with_bbox(
    doc: &Document,
    resources: &Dictionary,
    pattern_name: &[u8],
) -> Option<(Vec<ListedImage>, Option<f64>, Option<f64>)> {
    let Ok(patterns) = resources.get(b"Pattern") else {
        return None;
    };
    let patterns = resolve_obj(doc, patterns);
    let Ok(pattern_dict) = patterns.as_dict() else {
        return None;
    };
    let Ok(pref) = pattern_dict.get(pattern_name) else {
        return None;
    };
    let pattern_obj = resolve_obj(doc, pref);
    let (bbox_w, bbox_h) = pattern_bbox_size(doc, &pattern_obj);
    let pr = pattern_resources(doc, &pattern_obj)?;
    let mut out = Vec::new();
    let mut index = 0u32;
    append_xobject_images(doc, &pr, &mut out, &mut index);
    Some((out, bbox_w, bbox_h))
}

fn pattern_bbox_size(doc: &Document, pattern_obj: &Object) -> (Option<f64>, Option<f64>) {
    let dict = match pattern_obj {
        Object::Stream(s) => &s.dict,
        Object::Dictionary(d) => d,
        _ => return (None, None),
    };
    let Ok(bbox_obj) = dict.get(b"BBox") else {
        return (None, None);
    };
    let resolved = resolve_obj(doc, bbox_obj);
    let Ok(arr) = resolved.as_array() else {
        return (None, None);
    };
    if arr.len() < 4 {
        return (None, None);
    }
    let nums: Vec<f64> = arr
        .iter()
        .filter_map(|o| match resolve_obj(doc, o) {
            Object::Integer(i) => Some(i as f64),
            Object::Real(f) => Some(f),
            _ => None,
        })
        .collect();
    if nums.len() < 4 {
        return (None, None);
    }
    (
        Some((nums[2] - nums[0]).abs()),
        Some((nums[3] - nums[1]).abs()),
    )
}

/// Enumerate Image XObjects: page Resources/XObject first, then each Pattern's
/// nested XObject images (stable order for index matching).
fn list_page_images(doc: &Document, resources: &Dictionary) -> Vec<ListedImage> {
    let mut out = Vec::new();
    let mut index = 0u32;

    append_xobject_images(doc, resources, &mut out, &mut index);

    if let Ok(patterns) = resources.get(b"Pattern") {
        let patterns = resolve_obj(doc, patterns);
        if let Ok(pattern_dict) = patterns.as_dict() {
            for (_name, pref) in pattern_dict.iter() {
                let pattern_obj = resolve_obj(doc, pref);
                let pattern_resources = pattern_resources(doc, &pattern_obj);
                if let Some(pr) = pattern_resources.as_ref() {
                    append_xobject_images(doc, pr, &mut out, &mut index);
                }
            }
        }
    }

    out
}

fn pattern_resources(doc: &Document, pattern_obj: &Object) -> Option<Dictionary> {
    // Pattern can be a dictionary or a stream (tiling pattern with paint content).
    let dict = match pattern_obj {
        Object::Stream(s) => &s.dict,
        Object::Dictionary(d) => d,
        _ => return None,
    };
    let resources = dict.get(b"Resources").ok()?;
    let resolved = resolve_obj(doc, resources);
    resolved.as_dict().ok().cloned()
}

fn append_xobject_images(
    doc: &Document,
    resources: &Dictionary,
    out: &mut Vec<ListedImage>,
    index: &mut u32,
) {
    let Ok(xobjects) = resources.get(b"XObject") else {
        return;
    };
    let xobjects = resolve_obj(doc, xobjects);
    let Ok(xobject_dict) = xobjects.as_dict() else {
        return;
    };

    for (_name, xobj_ref) in xobject_dict.iter() {
        let xobj = resolve_obj(doc, xobj_ref);
        let Ok(stream) = xobj.as_stream() else {
            continue;
        };
        let dict = &stream.dict;
        let subtype = dict.get(b"Subtype").ok().and_then(|o| {
            if let Object::Name(ref n) = o {
                Some(String::from_utf8_lossy(n).to_string())
            } else {
                None
            }
        });
        if subtype.as_deref() != Some("Image") {
            continue;
        }
        let width = get_int(dict, b"Width").unwrap_or(0) as u32;
        let height = get_int(dict, b"Height").unwrap_or(0) as u32;
        if width == 0 || height == 0 {
            continue;
        }
        *index = index.saturating_add(1);
        out.push(ListedImage {
            index: *index,
            width,
            height,
            stream: stream.clone(),
        });
    }
}

fn extracted_from_stream(stream: Stream, image_index: u32) -> ExtractedImage {
    let dict = &stream.dict;
    let width = get_int(dict, b"Width").unwrap_or(0) as u32;
    let height = get_int(dict, b"Height").unwrap_or(0) as u32;
    let bpc = get_int(dict, b"BitsPerComponent").unwrap_or(8) as u8;

    let color_space = dict
        .get(b"ColorSpace")
        .ok()
        .and_then(|o| match o {
            Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "DeviceRGB".to_string());

    let filter = dict
        .get(b"Filter")
        .ok()
        .and_then(|o| match o {
            Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
            _ => None,
        })
        .unwrap_or_default();

    let data = if filter == "DCTDecode" {
        stream.content.clone()
    } else {
        stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone())
    };

    let bbox = BoundingBox::new(Some(0), 0.0, 0.0, width as f64, height as f64);

    ExtractedImage {
        chunk: ImageChunk {
            bbox,
            index: Some(image_index),
            level: None,
            source: Default::default(),
        },
        data,
        width,
        height,
        color_space,
        bits_per_component: bpc,
        filter,
    }
}

fn resolve_obj(doc: &Document, obj: &Object) -> Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).cloned().unwrap_or(Object::Null),
        other => other.clone(),
    }
}

fn get_int(dict: &lopdf::Dictionary, key: &[u8]) -> Option<i64> {
    dict.get(key).ok().and_then(|o| match o {
        Object::Integer(i) => Some(*i),
        Object::Real(f) => Some(*f as i64),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    #[test]
    fn test_extract_no_images() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => dictionary! {},
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let pages = doc.get_pages();
        let (&page_num, &page_id) = pages.iter().next().unwrap();
        let chunks = extract_image_chunks(&doc, page_num, page_id).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_extract_image_chunk() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let img_id = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 100,
                "Height" => 50,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            vec![0u8; 100],
        ));

        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! {
                "Im1" => img_id,
            },
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources_id,
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let pages = doc.get_pages();
        let (&page_num, &page_id) = pages.iter().next().unwrap();
        let chunks = extract_image_chunks(&doc, page_num, page_id).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].index, Some(1));
    }

    #[test]
    fn test_pattern_nested_image_index() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let img_id = doc.add_object(Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 200,
                "Height" => 100,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            vec![1u8; 200],
        ));

        let pattern_id = doc.add_object(dictionary! {
            "Type" => "Pattern",
            "PatternType" => 1,
            "PaintType" => 1,
            "TilingType" => 1,
            "BBox" => vec![0.into(), 0.into(), 200.into(), 100.into()],
            "XStep" => 200,
            "YStep" => 100,
            "Resources" => dictionary! {
                "XObject" => dictionary! {
                    "X1" => img_id,
                },
            },
        });

        let resources = dictionary! {
            "Pattern" => dictionary! {
                "P1" => pattern_id,
            },
        };

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => resources.clone(),
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let idx = find_pattern_image_index(&doc, &resources, b"P1");
        assert_eq!(idx, Some(1));

        let pages = doc.get_pages();
        let (&_page_num, &page_id) = pages.iter().next().unwrap();
        let extracted = extract_image_data(&doc, page_id, 1).unwrap().unwrap();
        assert_eq!(extracted.width, 200);
        assert_eq!(extracted.height, 100);
    }
}
