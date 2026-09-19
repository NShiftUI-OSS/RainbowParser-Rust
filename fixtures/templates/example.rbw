use Screen@1.0.0
use #{UsePin}

Screen(
  title: "Teste",
  variant: .name
) {
  Text(text: "Teste")

  #{PlaceholderPluginName}

  Button(
    title: "Teste",
    state: #{PlaceholderParamName}
  ) {
    OnClick {
      SendHTTPRequest()
    }
  }
}
