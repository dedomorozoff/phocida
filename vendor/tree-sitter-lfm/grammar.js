/**
 * Tree-sitter grammar for Lazarus form files (`.lfm`).
 *
 * Written against the corpus shipped with Lazarus (1,716 files, 546,743 lines,
 * 66 distinct line shapes; see tools/lfm_shapes.py), so every alternative here
 * corresponds to something that actually occurs:
 *
 *   object Form1: TForm1        inherited Frame1: TFrame1     inline Edit1: TEdit
 *   object Tab1: TTabSheet[0]                                 object TSynGutterMarks
 *     Left = 0                  Caption = 'a'#13#10'b'        Hint = []
 *     OnClick = Button1Click    Style = [cbStandard, cbExtended]
 *     ColWidths = (             Strings = (                   Items = <
 *       36                        'a'                           item
 *     )                         )                                 Caption = 'x'
 *     Bitmap = {                                                end>
 *       4C6905000000            }                             >
 *
 * There are no comments in this format: `{` opens a binary blob.
 */

module.exports = grammar({
  name: "lfm",

  extras: ($) => [/\s+/],

  word: ($) => $.identifier,

  rules: {
    // A form file holds exactly one root node; repeat1 keeps a truncated or
    // concatenated file parseable instead of failing outright.
    form: ($) => repeat1($.component),

    // Visible on purpose: were this rule hidden (`_node`) every nested object's
    // tokens would flatten into `form`'s children and the kind/name/class fields
    // would be dropped, so no query could tell a component from its property.
    component: ($) =>
      seq(
        field("kind", $._nodeKeyword),
        field("name", $.identifier),
        optional(seq(":", field("class", $.identifier))),
        optional(seq("[", field("index", $.number), "]")),
        repeat($._member),
        $.end,
      ),

    _nodeKeyword: ($) => choice($.object, $.inherited, $.inline),

    _member: ($) => choice($.component, $.property),

    property: ($) =>
      seq(field("name", $.identifier), "=", field("value", $._value)),

    _value: ($) =>
      choice($.number, $.string, $.identifier, $.set, $.list, $.itemList, $.binary),

    // prec.left: without it the parser cannot decide inside a `(...)` list
    // whether the next string literal continues this string or starts a new item.
    string: ($) => prec.left(repeat1(choice($.stringLiteral, $.charCode))),

    stringLiteral: () => /'(''|[^'])*'/,

    charCode: () => /#\d+/,

    number: () => choice(/-?\d+(\.\d+)?([eE][-+]?\d+)?/, /\$[0-9A-Fa-f]+/),

    identifier: () => /[A-Za-z_][A-Za-z0-9_.]*/,

    set: ($) => seq("[", optional(commaSeparated($._value)), "]"),

    list: ($) => seq("(", repeat($._value), ")"),

    // `<` `>` wrap a collection of `item` blocks; `<>` is the empty form and
    // the last item normally closes with `end>` on one line.
    itemList: ($) => seq("<", repeat($.item), ">"),

    item: ($) => seq($.itemKeyword, repeat($._member), $.end),

    // Binary property data: runs of hex digits, one line per 64 characters.
    binary: ($) => seq("{", repeat($.hexData), "}"),

    hexData: () => /[0-9A-Fa-f]+/,

    // `word: $.identifier` (see the grammar-level option) gives these the
    // word-boundary behaviour tree-sitter's regexes cannot express: a keyword
    // wins only when the identifier token does not match a *longer* string at
    // the same position, so `ObjectTypes`, `ItemHeight` and `Items.Strings`
    // stay identifiers while a bare `end` is the keyword.
    object: () => token(prec(1, /object/i)),
    inherited: () => token(prec(1, /inherited/i)),
    inline: () => token(prec(1, /inline/i)),
    itemKeyword: () => token(prec(1, /item/i)),
    end: () => token(prec(1, /end/i)),
  },
});

function commaSeparated(rule) {
  return seq(rule, repeat(seq(",", rule)), optional(","));
}
