; @start = token after which lines indent, @end = keyword returning to the opener's indent.
;
; Only constructs with a closing token belong here: an @indent range ends at the closing
; keyword, so lines in between keep indenting. Open-ended sections (`var`, `uses`, `const`,
; visibility specifiers) have no closer — their node ends on the last declaration, which
; would outdent the next one, so those are driven by increase/decrease_indent_pattern.

; -- begin/end blocks

(block
  (kBegin) @start
  (kEnd) @end) @indent

; -- Type bodies: class, record, interface, helper (records parse as declClass + kRecord)

(declClass
  (declSection) @end) @indent

(declClass
  (kEnd) @end) @indent

(declHelper
  (declSection) @end) @indent

(declHelper
  (kEnd) @end) @indent

(declIntf
  (kEnd) @end) @indent

; -- Single-statement bodies: the branch ends at its statement, so the line after it
; returns to the controlling statement's indent.

(if
  (kThen) @start) @indent

(ifElse
  (kElse) @start) @indent

(for
  (kDo) @start) @indent

(foreach
  (kDo) @start) @indent

(while
  (kDo) @start) @indent

(with
  (kDo) @start) @indent

; -- then/do followed by `begin` on its own line: begin sits level with the statement

(if
  (kThen) @start
  (block
    (kBegin)) @end) @indent

(ifElse
  (kThen) @start
  (kElse) @end) @indent

(ifElse
  (kThen) @start
  (block
    (kBegin)) @end) @indent

(ifElse
  (kElse) @start
  (block
    (kBegin)) @end) @indent

(for
  (kDo) @start
  (block
    (kBegin)) @end) @indent

(foreach
  (kDo) @start
  (block
    (kBegin)) @end) @indent

(while
  (kDo) @start
  (block
    (kBegin)) @end) @indent

(with
  (kDo) @start
  (block
    (kBegin)) @end) @indent

; -- Statement bodies with a closer

(repeat
  (kRepeat) @start
  (kUntil) @end) @indent

(case
  (kOf) @start
  [(kElse) (kOtherwise)] @end) @indent

(case
  (kOf) @start
  (kEnd) @end) @indent

(case
  [(kElse) (kOtherwise)] @start
  (kEnd) @end) @indent

(try
  (kTry) @start
  [
    (kExcept)
    (kFinally)
  ] @end) @indent

(try
  [
    (kExcept)
    (kFinally)
  ] @start
  (kEnd) @end) @indent

(asm
  (kAsm) @start
  (kEnd) @end) @indent

; -- Bracketed continuations

(_
  "(" @start
  ")" @end) @indent

(_
  "[" @start
  "]" @end) @indent
