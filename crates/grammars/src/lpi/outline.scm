; Units of a project (`<Units>` in .lpi, `<Files>` in .lpk). The item is the
; block that carries the `<UnitName>`, so `Unit0`/`Item3` become one outline
; entry each, named after the unit.
(element
  (selfClosingTag
    name: (name) @_unitName
    (#eq? @_unitName "UnitName")
    (attribute value: (quotedString content: (content) @name)))) @item

; Required packages: the `<Item>` block naming one dependency.
(element
  (selfClosingTag
    name: (name) @_packageName
    (#eq? @_packageName "PackageName")
    (attribute value: (quotedString content: (content) @name)))) @item

; The project title in .lpi, the package's own name in .lpk.
(element
  (selfClosingTag
    name: (name) @_title
    (#any-of? @_title "Title" "Name")
    (attribute value: (quotedString content: (content) @name)))) @item

; Build output, scoped to `<Target>` because `<Filename>` also appears in
; every unit block and would otherwise duplicate all of them.
(element
  open: (startTag name: (name) @type (#eq? @type "Target"))
  (element
    (selfClosingTag
      name: (name) @_filename
      (#eq? @_filename "Filename")
      (attribute value: (quotedString content: (content) @name))))) @item
