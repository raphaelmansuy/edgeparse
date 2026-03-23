//! PDF image extraction — find and extract inline/XObject images.

use lopdf::{Document, Object};

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

    // Get Resources → XObject dictionary
    let resources = match page_dict.get(b"Resources") {
        Ok(r) => resolve_obj(doc, r),
        Err(_) => return Ok(Vec::new()),
    };

    let resources_dict = match resources.as_dict() {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };

    let xobjects = match resources_dict.get(b"XObject") {
        Ok(x) => resolve_obj(doc, x),
        Err(_) => return Ok(Vec::new()),
    };

    let xobject_dict = match xobjects.as_dict() {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };

    let mut chunks = Vec::new();
    let mut index = 0u32;

    for (_name, xobj_ref) in xobject_dict.iter() {
        let xobj = resolve_obj(doc, xobj_ref);
        if let Ok(stream) = xobj.as_stream() {
            let dict = &stream.dict;

            // Check if this is an Image XObject (Subtype = Image)
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

            let width = get_int(dict, b"Width").unwrap_or(0) as f64;
            let height = get_int(dict, b"Height").unwrap_or(0) as f64;

            if width <= 0.0 || height <= 0.0 {
                continue;
            }

            index += 1;

            // Create bbox — position will be refined using content stream cm/Do operators
            // For now, use placeholder position based on image dimensions
            let bbox = BoundingBox::new(Some(page_number), 0.0, 0.0, width, height);

            chunks.push(ImageChunk {
                bbox,
                index: Some(index),
                level: None,
            });
        }
    }

    Ok(chunks)
}

/// Get raw image data for a specific XObject.
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

    let xobjects = match resources_dict.get(b"XObject") {
        Ok(x) => resolve_obj(doc, x),
        Err(_) => return Ok(None),
    };

    let xobject_dict = match xobjects.as_dict() {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    let mut current_index = 0u32;

    for (_name, xobj_ref) in xobject_dict.iter() {
        let xobj = resolve_obj(doc, xobj_ref);
        if let Ok(stream) = xobj.as_stream() {
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

            current_index += 1;
            if current_index != image_index {
                continue;
            }

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
                // JPEG data — use raw content
                stream.content.clone()
            } else {
                // Try to decompress
                stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone())
            };

            let bbox = BoundingBox::new(Some(0), 0.0, 0.0, width as f64, height as f64);

            return Ok(Some(ExtractedImage {
                chunk: ImageChunk {
                    bbox,
                    index: Some(image_index),
                    level: None,
                },
                data,
                width,
                height,
                color_space,
                bits_per_component: bpc,
                filter,
            }));
        }
    }

    Ok(None)
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
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
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
        let (&page_num, &pid) = pages.iter().next().unwrap();
        let chunks = extract_image_chunks(&doc, page_num, pid).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_extract_image_chunk() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        // Create a fake image XObject
        let img_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 100,
                "Height" => 200,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            vec![0u8; 100 * 200 * 3], // Fake pixel data
        );
        let img_id = doc.add_object(img_stream);

        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! {
                "Im1" => img_id,
            },
        });

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
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
        let (&page_num, &pid) = pages.iter().next().unwrap();
        let chunks = extract_image_chunks(&doc, page_num, pid).unwrap();
        assert_eq!(chunks.len(), 1);
    }
}
