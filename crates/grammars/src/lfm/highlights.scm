; Node kinds and the `end` that closes them.
[
  (object)
  (inherited)
  (inline)
  (itemKeyword)
  (end)
] @keyword

; `object Button1: TButton` — the class reads as a type, the instance name as the
; thing being configured, same as a property name.
(component class: (identifier) @type)
(component name: (identifier) @property)
(property name: (identifier) @property)

(string) @string
(number) @number
(binary (hexData) @string)

; A bare identifier value is either an enumerated constant (`Position = poDefault`)
; or, under an event property, the method the designer wired to it — `OnCreate`, or
; `OKButton.OnClick` when another component's event is set from here. The predicates
; make the two patterns mutually exclusive, so a node never carries both names.
(property
  name: (identifier) @property
  (#not-match? @property "(^|.)On[A-Za-z]+$")
  value: (identifier) @constant)

(property
  name: (identifier) @property
  (#match? @property "(^|.)On[A-Za-z]+$")
  value: (identifier) @function.method)

["=" ","] @punctuation.delimiter
["[" "]" "(" ")" "<" ">" "{"] @punctuation.bracket
