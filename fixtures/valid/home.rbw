Screen(name: "Home") {
  Button(title: "Entrar", variant: .primary)
  .events {
    OnTap {
      Navigate(to: "Dashboard")
    }
  }
}
