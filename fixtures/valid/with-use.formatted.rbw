use Screen@1.0.0
use Button@2.1.0

Screen(name: "Home") {
  Button(
    title: "Entrar",
    variant: .primary
  )
  .events {
    OnTap {
      Navigate(to: "Dashboard")
    }
  }
}
