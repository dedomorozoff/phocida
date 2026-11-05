; Children of an element sit one level in, from the `>` that opens the block
; to the `>` that closes it. Self-closing tags have no pair, which is why the
; pattern is on `element` and not on the tags themselves.
(element
  open: (startTag ">" @start)
  close: (endTag ">" @end)) @indent
