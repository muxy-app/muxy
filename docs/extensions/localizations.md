# Localizations

Muxy is in English. A language pack is an extension that adds app languages.
Users find packs with **Settings → Appearance → Browse language extensions…**
and pick one under **App language**. Text a pack doesn't translate stays in
English.

```json
{
  "name": "muxy-german",
  "version": "1.0.0",
  "muxy": {
    "localizations": [
      { "id": "de", "language": "de", "title": "Deutsch", "bundle": "localization/German.bundle" }
    ]
  }
}
```

| Field | Notes |
| --- | --- |
| `id` | Unique within the extension. |
| `language` | Language tag such as `de` or `pt-BR`. Must match the `.lproj` folder name. |
| `title` | Shown in Settings, ideally in the language itself. |
| `bundle` | A folder ending in `.bundle` inside the extension. |

## Bundle layout

```
localization/
  German.bundle/
    Info.plist
    de.lproj/
      Localizable.strings
      Localizable.stringsdict    (optional, for plurals)
```

`Localizable.strings` maps English text to translations:

```
"Apply Layout" = "Layout anwenden";
"Tab %lld" = "Tab %lld";
```

- The keys are the app's English strings. The full list is
  [`crates/muxy-app/localization/en.lproj/Localizable.strings`](../../crates/muxy-app/localization/en.lproj/Localizable.strings).
- Placeholders such as `%@` and `%lld` must match the key's, in type and
  position. Positional forms like `%1$@` may reorder them. A bundle with a
  mismatch is rejected.
- Each file can be up to 4 MiB. The bundle must not contain executable code.
- Load errors go to the pack's log in **Settings → Extensions**.

Muxy 2 uses new English strings, so packs made for 1.x load but translate only
part of the app.
