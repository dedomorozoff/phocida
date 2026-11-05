; @start = the token after which following lines indent, @end = the token that
; returns to @start's own column.

; A component (`object … end`) and an `item … end` block inside a collection both
; close with `end`, so the range is fully tokenised and no regex patterns are needed.
(component
  [(object) (inherited) (inline)] @start
  (end) @end) @indent

(item
  (itemKeyword) @start
  (end) @end) @indent

; Multi-line values: sets, lists, item collections and binary blobs keep wrapping one
; level deeper than the property that opened them.
(_
  "[" @start
  "]" @end) @indent

(_
  "(" @start
  ")" @end) @indent

(_
  "<" @start
  ">" @end) @indent

(binary
  "{" @start
  "}" @end) @indent
