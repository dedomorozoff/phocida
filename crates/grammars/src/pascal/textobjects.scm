; -- Functions

(defProc) @function.around

(defProc
  body: (block
    (kBegin)
    (_)* @function.inside
    (kEnd)))

(declProc) @function.around

; -- Classes and records

(declClass) @class.around

(declClass
  [
    (declField)
    (declProc)
    (declProp)
    (declSection)
    (declVars)
    (declConsts)
    (declTypes)
  ] @class.inside)

; -- Comments

(comment) @comment.around

; No conditional/loop text objects: `TextObject::from_capture_name`
; (crates/language_core/src/grammar.rs:156) only understands function, class and
; comment, and any other capture name is logged as unrecognized on every load.
