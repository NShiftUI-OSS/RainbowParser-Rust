// Concrete SemVer pins (version required)
use Screen@1.0.0
use Button@2.1.0
use ShowToast@1.3.0
use SendHTTPRequest@1.0.0
use Navigate@1.0.0
use PromoBanner@1.0.0

// Template pin: expand map value must be Name@MAJOR.MINOR.PATCH
use #{UsePin}

// Full-line template (map value is the entire use line)
#{FullUseLine}

// Root screen
Screen(
  title: "Rainbow showcase",
  subtitle: "quote: \" slash: \\ newline:\nhere",
  count: 42,
  rating: 4.5,
  enabled: true,
  disabled: false,
  missing: null,
  variant: .primary,
  tags: ["home", "demo", 1, null,],
  // Rainbow object value (native DSL, parentheses) — NOT JSON
  meta: (id: "home", "dash-key": false, nested: (ok: true)),
  // JSON must use @JSON(...)
  configJson: @JSON({"theme": "dark", "retries": 3}),
  configYaml: @YAML(enabled: true
items:
  - a
  - b),
  treeXml: @XML(<user id="1"><name>Ada</name></user>),
  bodyHtml: @HTML(<div class="card">Hello</div>),
  notesMd: @MARKDOWN(# Title

Hello **world**),
  // Whole-blob template hole inside @JSON(...); map value is raw JSON text
  blobJson: @JSON(#{JsonBlob}),
  state: #{PlaceholderParam}
) {
  Text(text: "Hello")

  // Node placeholder: map value is the bare plugin name (no version suffix)
  #{PluginNode}

  PromoBanner(title: "Sale")

  Button(
    title: "Entrar",
    variant: .primary,
    loading: false
  ) {
    OnTap {
      Navigate(to: "Dashboard")

      ShowToast(message: "ok")

      SendHTTPRequest(
        url: "https://example.com",
        method: .POST,
        body: @JSON({"id": 1})
      )
    }

    OnSuccess {
      ShowToast(message: "done")
    }
  }

  EmptyArgs()

  EmptyBlock() {
  }
}
