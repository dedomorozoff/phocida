(declProc
  [
    (kProcedure)
    (kFunction)
    (kConstructor)
    (kDestructor)
    (kOperator)
  ] @context
  name: (identifier) @name) @item

(declProc
  [
    (kProcedure)
    (kFunction)
    (kConstructor)
    (kDestructor)
    (kOperator)
  ] @context
  name: (genericTpl entity: (identifier) @name)) @item

(declProc
  [
    (kProcedure)
    (kFunction)
    (kConstructor)
    (kDestructor)
    (kOperator)
  ] @context
  name: (genericDot
    lhs: (identifier) @context
    rhs: (identifier) @name)) @item

(declType
  name: (identifier) @name) @item

(declType
  name: (genericTpl entity: (identifier) @name)) @item

(declProp
  name: (identifier) @name) @item

(declVar
  name: (identifier) @name) @item

(declConst
  name: (identifier) @name) @item

(unit
  (moduleName
    (identifier) @name)) @item

(program
  (moduleName
    (identifier) @name)) @item

(library
  (moduleName
    (identifier) @name)) @item

(interface
  (kInterface) @name) @item

(implementation
  (kImplementation) @name) @item

(comment) @annotation
