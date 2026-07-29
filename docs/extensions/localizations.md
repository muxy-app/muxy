# Localizations

Muxy includes English. An enabled extension can add one or more app languages by declaring `muxy.localizations`. The user chooses a provider in **Settings → Interface → Language**. If several enabled extensions provide the same language, each appears separately with its extension name.

Translations do not belong in `package.json`. The manifest contains only provider metadata and a path:

```json
{
  "name": "muxy-german",
  "version": "1.0.0",
  "scripts": {
    "build": "vite build && node scripts/copy-manifest.mjs && cp -R localization dist/localization"
  },
  "muxy": {
    "localizations": [
      {
        "id": "de",
        "language": "de",
        "title": "Deutsch",
        "bundle": "localization/German.bundle"
      }
    ]
  }
}
```

| Field | Meaning |
| --- | --- |
| `id` | Provider ID unique inside the extension. Letters, digits, `.`, `_`, and `-` are accepted. |
| `language` | BCP 47 language identifier such as `de`, `fr`, or `pt-BR`. It must exactly match the `.lproj` directory name. |
| `title` | Name shown in Settings, preferably written in the language itself. |
| `bundle` | Relative path inside the installed `dist/` output to a resource-only `.bundle` directory. |

## Bundle layout

The `.bundle` is an Apple resource directory, not executable code and not a compiled framework. It must be copied into `dist/`, but it does not need to be linked into the extension's JavaScript build.

```text
localization/
  German.bundle/
    Info.plist
    de.lproj/
      Localizable.strings
      Localizable.stringsdict
```

`Localizable.strings` contains the normal translations:

```text
"Settings" = "Einstellungen";
"Open Project" = "Projekt öffnen";
"Could not remove %@" = "Entfernen von %@ fehlgeschlagen";
```

`Localizable.stringsdict` is optional and can provide plural rules. At least one of `Localizable.strings` or `Localizable.stringsdict` must exist under the declared language's `.lproj` directory.

### Format placeholders

Every placeholder in a translated value must match the placeholder at the same argument position in its key. Muxy rejects the whole bundle otherwise, because a mismatched placeholder makes Foundation read an argument that was never passed.

```text
"Created branch %@" = "Zweig %@ erstellt";        ✅ same placeholder
"%@ (%@)"           = "%2$@ – %1$@";              ✅ reordered with positional placeholders
"%@ (%@)"           = "%1$@";                     ✅ dropping a trailing argument is allowed
"Created branch %@" = "Zweig %@ %@ erstellt";     ❌ adds an argument the app never passes
"%lld changes"      = "%@ Änderungen";            ❌ integer argument read as an object
"Created branch %@" = "Zweig %s erstellt";        ❌ object argument read as a C string
"Settings"          = "Einstellungen %@";         ❌ key takes no arguments
```

Use positional placeholders (`%1$@`, `%2$lld`) whenever the translation needs a different word order. `%ld` and `%lld` are interchangeable, as are `%d` and `%u` with their `h`/`hh` variants; `%*d`-style widths that consume an extra argument are rejected. In `Localizable.stringsdict`, the same rule applies to `NSStringLocalizedFormatKey` and to every plural variant inside each variable.

A minimal `Info.plist` is:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.example.muxy-german.localization</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>de</string>
</dict>
</plist>
```

Do not add `CFBundleExecutable`. Muxy rejects localization bundles that declare executable code, rejects paths that leave the extension directory, validates the property-list catalogs, and limits each catalog to 4 MiB. Muxy reads the bundle only through Foundation's string-localization APIs and never loads it as executable code.

## Fallback and lifecycle

Missing translation keys fall back to Muxy's English source text. If the selected provider is disabled, removed, or fails validation, Muxy temporarily uses English while preserving the selection. Enabling the provider again restores it automatically.

Language changes apply immediately to Muxy's native interface, including built-in search indexes and generated
status text. Search also retains English and technical terms as fallbacks. Extension-authored webview content,
extension names, command titles, and extension settings remain owned by each extension and are not translated by an
app-language provider.

The current English `Localizable.strings` file in the Muxy repository is the source template for translation extensions. Keep format placeholders unchanged and translate the value on the right-hand side.
