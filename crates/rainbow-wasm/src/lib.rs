//! WebAssembly entry points for the Node backend and the React frontend.
//!
//! The host-testable API lives in [`api`]. `bindings` is compiled only for
//! `wasm32` and re-exports that API through `wasm-bindgen`.

mod api;

pub use api::{expand, format, parse, validate, validate_registry, version};

#[cfg(target_arch = "wasm32")]
mod bindings;

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn parse_button_ok() {
        let json = crate::parse(r#"Button(title: "Entrar")"#);
        assert!(json.contains(r#""ok":true"#), "{json}");
    }

    #[wasm_bindgen_test]
    fn expand_substitutes_placeholder() {
        let json = crate::expand(
            "Button(title: #{Title})",
            r#"{"Title":"\"Entrar\""}"#,
            true,
        );
        assert!(json.contains(r#""ok":true"#), "{json}");
        assert!(json.contains("Entrar"), "{json}");
    }
}
