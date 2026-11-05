; The form's component tree, which is what the Lazarus Object Inspector shows.
; A node without a class (`object TSynGutterMarks`) is left out: adding a second
; pattern for it would list every component twice, since a query matches on the
; fields it names and ignores the rest.
(component
  [(object) (inherited) (inline)] @context
  name: (identifier) @name
  class: (identifier) @type) @item
