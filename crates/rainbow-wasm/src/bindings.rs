use wasm_bindgen::prelude::*;

#[cfg(not(test))]
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = version)]
pub fn version() -> String {
    crate::version()
}

#[wasm_bindgen(js_name = parse)]
pub fn parse(source: &str) -> String {
    crate::parse(source)
}

#[wasm_bindgen(js_name = format)]
pub fn format(source: &str) -> String {
    crate::format(source)
}

#[wasm_bindgen(js_name = validate)]
pub fn validate(source: &str, concrete: bool) -> String {
    crate::validate(source, concrete)
}

#[wasm_bindgen(js_name = expand)]
pub fn expand(source: &str, map_json: &str, validate: bool) -> String {
    crate::expand(source, map_json, validate)
}

#[wasm_bindgen(js_name = validateRegistry)]
pub fn validate_registry(source: &str) -> String {
    crate::validate_registry(source)
}
