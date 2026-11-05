; `<CONFIG` pairs with the `>` of `</CONFIG>`, so jumping to the matching
; bracket lands on the end of the element's closing tag.
(element
  open: (startTag "<" @open)
  close: (endTag ">" @close))
