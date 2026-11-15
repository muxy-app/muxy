# S14 Terminal images

Programs show pictures in the terminal with the Kitty graphics protocol,
iTerm2 inline images, and Sixel. Muxy reads Kitty images from regular files
only, deletes temporary files after reading them, and answers the terminal's
device-attributes query with Sixel support.

Start: run `$E g $VM reset`, then:

1. Run `$E g $VM image-fixture`. Check: it prints `image fixtures written to`.
2. Press new tab `$E press $VM new-tab`, then `$E g $VM wait 'muxy@'`.

Tests continue from each other in order. Before each test, clear the screen:
`$E type $VM 'clear'`, `$E key $VM Return`, `sleep 1`.

`~/e2e/images/show <kind>` prints a text marker, then the image, then a second
marker. The PNG has four colored quarters: red, green, blue and yellow. The
Sixel image has purple, cyan, orange and white quarters.

## T14.1 Kitty graphics

1. `$E type $VM '~/e2e/images/show kitty'`, `$E key $VM Return`,
   `$E g $VM wait 'KITTY-AFTER'`.
2. **Shot** `t14-1-kitty`. Check: between the `KITTY-BEFORE` and `KITTY-AFTER`
   lines, a picture with red, green, blue and yellow quarters, about 16 columns
   wide. Not escape-code text.

## T14.2 iTerm2 inline image

1. `$E type $VM '~/e2e/images/show iterm'`, `$E key $VM Return`,
   `$E g $VM wait 'ITERM-AFTER'`.
2. **Shot** `t14-2-iterm`. Check: the same four-color picture between
   `ITERM-BEFORE` and `ITERM-AFTER`.

## T14.3 Sixel

1. `$E type $VM '~/e2e/images/show sixel'`, `$E key $VM Return`,
   `$E g $VM wait 'SIXEL-AFTER'`.
2. **Shot** `t14-3-sixel`. Check: between `SIXEL-BEFORE` and `SIXEL-AFTER`, a
   picture with purple, cyan, orange and white quarters.

## T14.4 Kitty image from a file; the file is kept

1. `$E type $VM '~/e2e/images/show kitty-file'`, `$E key $VM Return`,
   `$E g $VM wait 'FILE-AFTER'`.
2. **Shot** `t14-4-file`. Check: the four-color picture between `FILE-BEFORE`
   and `FILE-AFTER`.
3. Run `$E g $VM exists '~/e2e/images/photo.png'`. Check: `yes`.

## T14.5 Kitty image from a temporary file; the file is deleted

1. `$E type $VM '~/e2e/images/show kitty-temp'`, `$E key $VM Return`,
   `$E g $VM wait 'TEMP-AFTER'`.
2. Run `$E g $VM wait 'TEMP-DELETED'`. Check: it succeeds. If the screen shows
   `TEMP-KEPT` instead, mark FAIL.
3. **Shot** `t14-5-temp`. Check: the four-color picture between `TEMP-BEFORE`
   and `TEMP-AFTER`, then `TEMP-DELETED`.

## T14.6 A named pipe is refused without hanging

1. `$E type $VM '~/e2e/images/show fifo'`, `$E key $VM Return`,
   `$E g $VM wait 'FIFO-AFTER'`.
2. `$E type $VM 'echo after-fifo-$((4*9))'`, `$E key $VM Return`,
   `$E g $VM wait 'after-fifo-36'`.
3. **Shot** `t14-6-fifo`. Check: **no** picture between `FIFO-BEFORE` and
   `FIFO-AFTER`, and `after-fifo-36` is shown (the terminal still works).

## T14.7 Device attributes report Sixel

1. `$E type $VM '~/e2e/images/show da1'`, `$E key $VM Return`,
   `$E g $VM wait 'DA1='`.
2. Run `$E g $VM find 'DA1='`. Check: the line reads `DA1=[?62;4;22c`
   (the `4` means Sixel). Quote what you saw.

## T14.8 Images come back after the app restarts

1. `$E type $VM '~/e2e/images/show kitty'`, `$E key $VM Return`,
   `$E g $VM wait 'KITTY-AFTER'`.
2. Run `$E g $VM quit`, then `$E g $VM launch`. Wait: `sleep 3`.
3. **Shot** `t14-8-relaunched`. Check: the tab is restored and the four-color
   picture is still drawn between `KITTY-BEFORE` and `KITTY-AFTER`.

## T14.9 Images scroll with their text

Don't clear the screen before this test: it continues from T14.8's picture.

1. `$E type $VM 'seq 1 200; echo seq-done-$((10*10))'`, `$E key $VM Return`,
   `$E g $VM wait 'seq-done-100'`.
2. Press scroll up `$E press $VM scroll-page-up` 8 times, one command each.
   Don't use `find` or `wait` here: they read the live screen, not the
   scrolled-back view.
3. **Shot** `t14-9-scrolled`. Check: `KITTY-BEFORE` and `KITTY-AFTER` are both
   visible, and the picture is drawn between them, not left at its old place
   on screen over the numbers.

Needs verification before first use:
- T14.7: the DA1 reply comes from `crates/muxy-terminal/tests/images.rs`;
  confirm the shell prints it the same way in both guests.
- T14.8: whether the server keeps image placements for an app that reconnects
  (this is the regression the test is for); if it never did, record the result
  as a finding, not a test defect.
- T14.9: the number of page-ups depends on the window height.
