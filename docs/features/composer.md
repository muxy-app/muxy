# Composer

The composer is a text box for writing long prompts and commands, with files,
images, and dictation. It sends to the active terminal, or to every split in
the tab. Handy for AI agents.

## Use it

| Action | Default shortcut |
| --- | --- |
| Open or close | `Cmd+I` |
| Send and press Return | `Cmd+Return` |
| Send without Return | `Cmd+Shift+Return` |
| Close | `Esc` |

- Sending replaces anything already typed at the terminal's prompt.
- With text selected, only the selection is sent.
- **Send to All Split Panes** in the **…** menu sends to every terminal in the
  tab.
- A failed send keeps the draft.
- Each project keeps its own draft, saved across restarts.

## Files and images

Attach with **Attach File**, drag and drop, or paste.

- Files are inserted as quoted paths.
- Images appear as `[Image N]`. By default they are put on the clipboard and
  sent with `Ctrl+V`, which AI tools read. Set **Settings → Composer → Image
  submission** to paste file paths instead.
- For [remote](remote-servers.md) terminals, attachments are uploaded and the
  remote path is sent.

## Layout

The composer opens as a panel on the right or bottom, or as a floating box
that closes after sending. Switch from its **…** menu, which also has
**Clear After Sending**, **Clear on Close**, and **Reset Composer Size**.
`Cmd+=` and `Cmd+-` change its font size.

## Dictation

`Cmd+Shift+I` starts on-device dictation. Inside the composer, it types into
the draft. Elsewhere it types into the active terminal; press `Return` to
insert, `Space` to pause, or `Esc` to cancel.

Dictation needs an on-device speech language, plus microphone and speech
recognition access. Pick the language and **Send after recording** in
**Settings → Composer**.
