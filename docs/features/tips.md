# Tips

Muxy's built-in sidebar tips are stored in `Muxy/Resources/tips.json`. The file is bundled with the app, decoded once
per process, and never fetched from a remote service.

Each entry has exactly one field:

```json
{
  "description": "A concise, actionable tip."
}
```

Descriptions must contain non-whitespace text. They may contain inline Markdown links when a tip needs to reference
Muxy documentation. Do not add names, identifiers, categories, links, display rules, or other fields. Keep the JSON
order intentional because the previous and next buttons follow it. Every description is also a localization key and
must have a matching entry in `Muxy/Resources/Localization/en.lproj/Localizable.strings`.

Verify every feature claim against its executable code path before adding or changing a tip. State prerequisites that
change whether the action is available, qualify customizable shortcuts as defaults, and distinguish local, remote,
active, hidden, persistent, and recreated state where those differences affect the result. Documentation links under
`https://muxy.app/docs/` must map to a Markdown file under `docs/` with the same path. If existing behavior does not
support a proposed tip, change or omit the tip instead of changing the feature solely to make the tip true. A gesture
handler existing in source is not enough evidence that the gesture is reachable; gesture tips also require interaction
test coverage or confirmed manual behavior.

Muxy chooses a random starting entry once per app launch. It does not rotate tips automatically. Missing, malformed,
empty, or invalid catalogs are logged and leave the tip interface hidden.

Closing a tip asks for confirmation before hiding the interface. Tips can be restored from
**Settings → Interface → Sidebar → Show Tips**.
