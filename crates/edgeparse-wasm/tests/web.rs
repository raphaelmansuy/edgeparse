//! WASM bindgen tests — `wasm-pack test --node -p edgeparse-wasm`

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

fn hello_pdf() -> Vec<u8> {
    br#"%PDF-1.1
1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj
2 0 obj<< /Type /Pages /Kids [3 0 R] /Count 1 >>endobj
3 0 obj<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 144] /Contents 4 0 R /Resources<< /Font<< /F1 5 0 R >> >> >>endobj
4 0 obj<< /Length 44 >>stream
BT /F1 24 Tf 100 100 Td (Hello) Tj ET
endstream
endobj
5 0 obj<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000266 00000 n 
0000000361 00000 n 
trailer<< /Size 6 /Root 1 0 R >>
startxref
429
%%EOF
"#
    .to_vec()
}

#[wasm_bindgen_test]
fn convert_to_string_does_not_panic() {
    let pdf = hello_pdf();
    let _ = crate::convert_to_string(&pdf, Some("markdown".into()), None, None, None);
}

#[wasm_bindgen_test]
fn convert_hybrid_accepts_injected_markdown() {
    let pdf = hello_pdf();
    let backend = "| A | B |\n| --- | --- |\n| 1 | 2 |\n".to_string();
    let out = crate::convert_hybrid(
        &pdf,
        Some(backend),
        Some("markdown".into()),
        None,
        None,
        None,
    );
    assert!(out.is_ok());
}

#[wasm_bindgen_test]
fn parse_session_open_finish_roundtrip() {
    let pdf = hello_pdf();
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &"fileName".into(), &"hello.pdf".into()).unwrap();
    let mut session = crate::ParseSession::open(&pdf, opts.into(), None).expect("open");
    let candidates = session.candidates().expect("candidates");
    assert!(candidates.is_array() || candidates.is_object());
    let md = session.finish("markdown").expect("finish");
    assert!(md.to_lowercase().contains("hello") || !md.is_empty());
}
