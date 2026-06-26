use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = elykFetchText)]
    fn fetch_text_js(url: &str) -> js_sys::Promise;
}

#[derive(Deserialize)]
struct FetchResult {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    status: u16,
    #[serde(default)]
    status_text: String,
    #[serde(default)]
    text: String,
}

pub async fn fetch_text(url: &str) -> Result<String, String> {
    let value = JsFuture::from(fetch_text_js(url))
        .await
        .map_err(describe_js_error)?;
    let json = value
        .as_string()
        .ok_or_else(|| "fetch returned a non-string result".to_string())?;
    let result: FetchResult = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    if result.ok {
        Ok(result.text)
    } else {
        let status_text = result.status_text.trim();
        if status_text.is_empty() {
            Err(format!("HTTP {}", result.status))
        } else {
            Err(format!("HTTP {} {}", result.status, status_text))
        }
    }
}

fn describe_js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(&value, &JsValue::from_str("message"))
                .ok()
                .and_then(|message| message.as_string())
        })
        .unwrap_or_else(|| "JavaScript error".to_string())
}
