use Screen@1.0.0
use Button@2.1.0

Screen(name: "Home") {
  Button(title: "Entrar", variant: .primary) {
    OnTap {
      Navigate(to: "Dashboard")
    }
  }
}
