Screen(
  navigationBar: (title: "Feedback", theme: "amber", leading: "back"),
  scroll: (enabled: true, fastRefresh: true)
) {
  VerticalStack(style: (alignment: .leading, spacing: 12)) {
    Progress(value: 0.66)
    Spinner
    Separator
  }
}
