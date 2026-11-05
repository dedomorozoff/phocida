[(startTag name: (name) @tag)
 (selfClosingTag name: (name) @tag)
 (endTag (name) @tag)]

(attribute key: (name) @attribute)

(quotedString content: (content) @string)

; The declaration is the only processing instruction the format writes; the
; tokens are captured rather than the node so the attributes inside it keep
; their own colours.
["<?xml" "?>"] @keyword

["<" ">" "</" "/>" "="] @punctuation.bracket
