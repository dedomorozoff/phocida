; -- Keywords

[
  (kProgram)
  (kLibrary)
  (kUnit)
  (kUses)
  (kBegin)
  (kEnd)
  (kAsm)
  (kVar)
  (kThreadvar)
  (kConst)
  (kResourcestring)
  (kConstref)
  (kOut)
  (kType)
  (kLabel)
  (kExports)
  (kAbsolute)
  (kProperty)
  (kRead)
  (kWrite)
  (kImplements)
  (kDefault)
  (kNodefault)
  (kStored)
  (kIndex)
  (kDispId)
  (kClass)
  (kInterface)
  (kDispInterface)
  (kObject)
  (kRecord)
  (kObjcclass)
  (kObjccategory)
  (kObjcprotocol)
  (kArray)
  (kFile)
  (kString)
  (kSet)
  (kOf)
  (kHelper)
  (kPacked)
  (kGeneric)
  (kSpecialize)
  (kFunction)
  (kProcedure)
  (kConstructor)
  (kDestructor)
  (kOperator)
  (kReference)
  (kImplementation)
  (kInitialization)
  (kFinalization)
  (kPublished)
  (kPublic)
  (kProtected)
  (kPrivate)
  (kStrict)
  (kRequired)
  (kOptional)
  (kForward)
  (kStatic)
  (kVirtual)
  (kAbstract)
  (kSealed)
  (kDynamic)
  (kOverride)
  (kOverload)
  (kReintroduce)
  (kInherited)
  (kInline)
  (kStdcall)
  (kCdecl)
  (kCppdecl)
  (kPascal)
  (kRegister)
  (kMwpascal)
  (kExternal)
  (kName)
  (kMessage)
  (kDeprecated)
  (kExperimental)
  (kPlatform)
  (kUnimplemented)
  (kCvar)
  (kExport)
  (kFar)
  (kNear)
  (kSafecall)
  (kAssembler)
  (kNostackframe)
  (kInterrupt)
  (kNoreturn)
  (kIocheck)
  (kLocal)
  (kHardfloat)
  (kSoftfloat)
  (kMs_abi_default)
  (kMs_abi_cdecl)
  (kSaveregisters)
  (kSysv_abi_default)
  (kSysv_abi_cdecl)
  (kVectorcall)
  (kVarargs)
  (kWinapi)
  (kAlias)
  (kDelayed)
  (kFor)
  (kTo)
  (kDownto)
  (kIf)
  (kThen)
  (kElse)
  (kOtherwise)
  (kDo)
  (kWhile)
  (kRepeat)
  (kUntil)
  (kTry)
  (kExcept)
  (kFinally)
  (kRaise)
  (kOn)
  (kCase)
  (kWith)
  (kGoto)
] @keyword

; Technically operators, but they read as reserved words.
[
  (kOr)
  (kXor)
  (kDiv)
  (kMod)
  (kAnd)
  (kShl)
  (kShr)
  (kNot)
  (kIs)
  (kAs)
  (kIn)
  (kNil)
] @keyword

; -- Punctuation and operators

[
  "("
  ")"
  "["
  "]"
] @punctuation.bracket

[
  ";"
  ","
  ":"
  ".."
  (kEndDot)
] @punctuation.delimiter

[
  (kDot)
  (kAdd)
  (kSub)
  (kMul)
  (kFdiv)
  (kAssign)
  (kAssignAdd)
  (kAssignSub)
  (kAssignMul)
  (kAssignDiv)
  (kEq)
  (kLt)
  (kLte)
  (kGt)
  (kGte)
  (kNeq)
  (kAt)
  (kHat)
] @operator

; -- Literals and built-in constants

[
  (kTrue)
  (kFalse)
] @constant

(literalNumber) @number
(literalString) @string
(literalChar) @string

; `{$...}` directives are compiler instructions, not comments.
(pp) @keyword

(comment) @comment

; -- Declarations

(declType name: (identifier) @type)
(declType name: (genericTpl entity: (identifier) @type))

(declProc name: (identifier) @function)
(declProc name: (genericTpl entity: (identifier) @function))
(declProc name: (genericDot rhs: (identifier) @function.method))
(declProc name: (genericDot rhs: (genericTpl entity: (identifier) @function.method)))

(declProp name: (identifier) @property)

(declArg name: (identifier) @variable.parameter)
(exceptionHandler variable: (identifier) @variable.parameter)

(genericArg name: (identifier) @type.parameter)
(genericArg type: (typeref) @type)

(declVar name: (identifier) @variable)
(declField name: (identifier) @property)
(declConst name: (identifier) @constant)
(declEnumValue name: (identifier) @constant)

[
  (caseLabel)
  (label)
] @constant

; -- Type usage

(typeref) @type
(genericDot (identifier) @type)
(genericDot (genericTpl entity: (identifier) @type))

; -- Call sites
; Pascal allows parenthesis-free calls, so a bare identifier statement is
; assumed to be a procedure call.

(exprCall entity: (identifier) @function)
(exprCall entity: (exprTpl entity: (identifier) @function))
(exprCall entity: (exprDot rhs: (identifier) @function.method))
(exprCall entity: (exprDot rhs: (exprTpl entity: (identifier) @function.method)))

(statement (identifier) @function)
(statement (exprDot rhs: (identifier) @function.method))
(statement (exprTpl entity: (identifier) @function))
(statement (exprDot rhs: (exprTpl entity: (identifier) @function.method)))
